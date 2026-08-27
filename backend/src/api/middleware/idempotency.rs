//! Idempotency-Key Middleware
//!
//! Per system_design.md §7.4: Implements idempotent write operations
//! using the Idempotency-Key HTTP header.
//!
//! ## Flow:
//! 1. Check if Idempotency-Key header exists
//! 2. Query idempotency_keys table
//!    - Exists + same hash → Return cached response
//!    - Exists + different hash → Return 409 Conflict
//!    - Exists + response_code=0 → Return 202 Accepted (processing)
//!    - Not exists → Insert placeholder, proceed with request
//! 3. After request completes, update cached response
//!
//! ## Headers:
//! - `Idempotency-Key`: Client-provided unique key (UUID v4 recommended)
//! - `Idempotency-Replayed`: Set to "true" if response is from cache

use axum::{body::Body, http::Response, http::StatusCode};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

use crate::api::error::{ApiErrorBody, ApiErrorResponse, ApiErrorType};
use crate::entity::{idempotency_keys, IdempotencyKeys};

/// Header name for idempotency key
pub const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";

/// Header name indicating replayed response
pub const IDEMPOTENCY_REPLAYED_HEADER: &str = "idempotency-replayed";

/// Maximum retries for the insert-or-fetch loop (handles rare race conditions)
const MAX_IDEMPOTENCY_RETRIES: u8 = 3;

/// Zombie timeout in seconds — processing records older than this can be taken over
const ZOMBIE_TIMEOUT_SECONDS: i64 = 60;

/// Idempotency check result
#[derive(Debug)]
pub enum IdempotencyResult {
    /// No key provided, proceed normally
    NoKey,
    /// Proceed with request (new key)
    Proceed { key: String, request_hash: String },
    /// Return cached response
    Cached {
        status_code: StatusCode,
        body: serde_json::Value,
    },
    /// Request hash mismatch - conflict
    Conflict,
    /// Another request is processing this key
    Processing,
}

use chrono::Utc;

/// Check idempotency key and return appropriate action.
///
/// Uses a loop (not recursion) and CAS-based zombie takeover to prevent
/// double-execution under concurrent requests.
pub async fn check_idempotency(
    db: &DatabaseConnection,
    merchant_id: &str,
    idempotency_key: &str,
    request_path: &str,
    request_body: &[u8],
) -> Result<IdempotencyResult, sea_orm::DbErr> {
    let request_hash = compute_request_hash(request_body);

    for attempt in 0..MAX_IDEMPOTENCY_RETRIES {
        // Step 1: Try to insert a new placeholder (acquire lock)
        let new_record = idempotency_keys::ActiveModel {
            merchant_id: Set(merchant_id.to_string()),
            idempotency_key: Set(idempotency_key.to_string()),
            request_path: Set(request_path.to_string()),
            request_hash: Set(request_hash.clone()),
            response_code: Set(0), // 0 = Processing
            response_body: Set(serde_json::json!({})),
            ..Default::default() // created_at uses DB DEFAULT NOW()
        };

        match idempotency_keys::Entity::insert(new_record).exec(db).await {
            Ok(_) => {
                debug!(
                    idempotency_key = %idempotency_key,
                    "Created idempotency placeholder (lock acquired)"
                );
                return Ok(IdempotencyResult::Proceed {
                    key: idempotency_key.to_string(),
                    request_hash,
                });
            }
            Err(e) => {
                // Step 2: Check if it's a unique constraint violation
                let msg = e.to_string().to_lowercase();
                if !msg.contains("duplicate") && !msg.contains("unique") {
                    return Err(e); // Real DB error, propagate
                }

                // Step 3: Record exists — fetch and inspect
                let existing = IdempotencyKeys::find()
                    .filter(idempotency_keys::Column::MerchantId.eq(merchant_id))
                    .filter(idempotency_keys::Column::IdempotencyKey.eq(idempotency_key))
                    .one(db)
                    .await?;

                let record = match existing {
                    Some(r) => r,
                    None => {
                        // Rare edge case: Insert reported duplicate but Select found nothing
                        // (record deleted/expired between the two statements).
                        // Loop back to retry insert.
                        warn!(
                            idempotency_key = %idempotency_key,
                            attempt = attempt,
                            "Race condition: insert conflict but record not found, retrying"
                        );
                        continue;
                    }
                };

                // Step 4a: Still processing (response_code == 0)
                if record.response_code == 0 {
                    let now = Utc::now();
                    let duration = now.signed_duration_since(record.created_at);

                    if duration.num_seconds() < ZOMBIE_TIMEOUT_SECONDS {
                        // Within normal processing window
                        debug!(
                            idempotency_key = %idempotency_key,
                            elapsed_seconds = duration.num_seconds(),
                            "Request still processing"
                        );
                        return Ok(IdempotencyResult::Processing);
                    }

                    // Zombie detected — attempt CAS takeover
                    // The WHERE includes created_at to ensure only one concurrent
                    // request can win the takeover (Compare-And-Swap).
                    warn!(
                        idempotency_key = %idempotency_key,
                        elapsed_seconds = duration.num_seconds(),
                        "Zombie detected, attempting CAS takeover"
                    );

                    let update_result = idempotency_keys::Entity::update_many()
                        .col_expr(
                            idempotency_keys::Column::CreatedAt,
                            sea_orm::sea_query::Expr::value(Utc::now()),
                        )
                        .col_expr(
                            idempotency_keys::Column::RequestHash,
                            sea_orm::sea_query::Expr::value(request_hash.clone()),
                        )
                        .col_expr(
                            idempotency_keys::Column::RequestPath,
                            sea_orm::sea_query::Expr::value(request_path.to_string()),
                        )
                        .filter(idempotency_keys::Column::MerchantId.eq(merchant_id))
                        .filter(idempotency_keys::Column::IdempotencyKey.eq(idempotency_key))
                        .filter(idempotency_keys::Column::ResponseCode.eq(0))
                        // CAS condition: only succeed if created_at hasn't changed
                        .filter(idempotency_keys::Column::CreatedAt.eq(record.created_at))
                        .exec(db)
                        .await?;

                    if update_result.rows_affected > 0 {
                        info!(
                            idempotency_key = %idempotency_key,
                            "Zombie takeover successful (CAS)"
                        );
                        return Ok(IdempotencyResult::Proceed {
                            key: idempotency_key.to_string(),
                            request_hash,
                        });
                    }

                    // Another thread won the takeover — loop back to re-read
                    warn!(
                        idempotency_key = %idempotency_key,
                        "CAS takeover lost to another thread, retrying"
                    );
                    continue;
                }

                // Step 4b: Request hash mismatch → conflict
                if record.request_hash != request_hash {
                    warn!(
                        idempotency_key = %idempotency_key,
                        "Idempotency key reused with different request body"
                    );
                    return Ok(IdempotencyResult::Conflict);
                }

                // Step 4c: Completed request → return cached response
                info!(
                    idempotency_key = %idempotency_key,
                    status_code = record.response_code,
                    "Returning cached response"
                );
                return Ok(IdempotencyResult::Cached {
                    status_code: StatusCode::from_u16(record.response_code as u16)
                        .unwrap_or(StatusCode::OK),
                    body: record.response_body,
                });
            }
        }
    }

    // Exhausted retries — should never happen in practice
    Err(sea_orm::DbErr::Custom(
        "Idempotency check failed after max retries".to_owned(),
    ))
}

/// Update cached response after request completes
pub async fn update_idempotency_response(
    db: &DatabaseConnection,
    merchant_id: &str,
    idempotency_key: &str,
    status_code: StatusCode,
    response_body: &serde_json::Value,
) -> Result<(), sea_orm::DbErr> {
    // Use ActiveModel for DB-agnostic update
    let record = idempotency_keys::ActiveModel {
        merchant_id: Set(merchant_id.to_string()),
        idempotency_key: Set(idempotency_key.to_string()),
        response_code: Set(status_code.as_u16() as i32),
        response_body: Set(response_body.clone()),
        ..Default::default()
    };

    idempotency_keys::Entity::update(record).exec(db).await?;

    debug!(
        idempotency_key = %idempotency_key,
        status_code = %status_code,
        "Updated idempotency response cache"
    );

    Ok(())
}

/// Compute SHA256 hash of request body
fn compute_request_hash(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body);
    format!("{:x}", hasher.finalize())
}

/// Build conflict response
pub fn conflict_response() -> Response<Body> {
    let response = ApiErrorResponse {
        error: ApiErrorBody {
            error_type: ApiErrorType::IdempotencyError,
            code: "idempotency_conflict".to_string(),
            message: "Idempotency key was used with a different request body".to_string(),
            param: None,
            doc_url: Some("https://ironixpay.com/guide/errors#idempotency_conflict".to_string()),
        },
    };
    let body = serde_json::to_string(&response).unwrap_or_default();

    Response::builder()
        .status(StatusCode::CONFLICT)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

/// Build processing response (202 Accepted)
pub fn processing_response() -> Response<Body> {
    let response = ApiErrorResponse {
        error: ApiErrorBody {
            error_type: ApiErrorType::ApiError,
            code: "request_in_progress".to_string(),
            message: "A request with this idempotency key is currently being processed".to_string(),
            param: None,
            doc_url: None,
        },
    };
    let body = serde_json::to_string(&response).unwrap_or_default();

    Response::builder()
        .status(StatusCode::ACCEPTED)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

/// Build cached response with replay header
pub fn cached_response(status_code: StatusCode, body: &serde_json::Value) -> Response<Body> {
    let body_str = serde_json::to_string(body).unwrap_or_default();

    Response::builder()
        .status(status_code)
        .header("content-type", "application/json")
        .header(IDEMPOTENCY_REPLAYED_HEADER, "true")
        .body(Body::from(body_str))
        .unwrap()
}
