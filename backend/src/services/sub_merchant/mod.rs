//! Sub-Merchant Service
//!
//! Core business logic for sub-merchant (PSP) operations.
//! Called by admin, public, and internal API handlers.

use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::entity::{
    merchants, sub_merchants, transactions, Merchants, Network, SubMerchants, Transactions,
};
use crate::services::address::AddressManager;

/// Response DTO for sub-merchant operations.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SubMerchantResponse {
    /// Unique sub-merchant record ID (e.g. `sm_abc123`).
    #[schema(example = "sm_abc123")]
    pub id: String,
    /// The parent merchant's organization ID.
    pub parent_org_id: String,
    /// The unique code identifying this sub-merchant (e.g. `shop_tokyo`).
    #[schema(example = "shop_tokyo")]
    pub sub_merchant_code: String,
    /// Human-readable display name.
    #[schema(example = "Tokyo Branch")]
    pub display_name: String,
    /// The auto-generated child organization ID.
    pub child_org_id: String,
    /// Current status: `active` or `suspended`.
    pub status: sub_merchants::SubMerchantStatus,
    /// ISO 8601 creation timestamp.
    #[schema(example = "2026-01-15T08:30:00Z")]
    pub created_at: String,
    /// ISO 8601 last-updated timestamp.
    #[schema(example = "2026-03-20T10:00:00Z")]
    pub updated_at: String,
}

impl From<sub_merchants::Model> for SubMerchantResponse {
    fn from(sm: sub_merchants::Model) -> Self {
        Self {
            id: sm.id,
            parent_org_id: sm.parent_org_id,
            sub_merchant_code: sm.sub_merchant_code,
            display_name: sm.display_name,
            child_org_id: sm.child_org_id,
            status: sm.status,
            created_at: sm.created_at.to_rfc3339(),
            updated_at: sm.updated_at.to_rfc3339(),
        }
    }
}

/// Input for creating a sub-merchant.
#[derive(Debug)]
pub struct CreateSubMerchantInput {
    pub parent_org_id: String,
    pub sub_merchant_code: String,
    pub display_name: String,
}

/// Input for updating a sub-merchant.
#[derive(Debug, Deserialize)]
pub struct UpdateSubMerchantInput {
    pub display_name: Option<String>,
    pub status: Option<sub_merchants::SubMerchantStatus>,
}

/// Pagination parameters.
#[derive(Debug)]
pub struct Pagination {
    pub page: u64,
    pub page_size: u64,
}

/// Paginated result.
#[derive(Debug, Serialize)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

/// Per-sub-merchant stats entry.
#[derive(Debug, Serialize)]
pub struct SubMerchantStatsEntry {
    pub sub_merchant_code: String,
    pub display_name: String,
    pub status: sub_merchants::SubMerchantStatus,
    pub total_volume: String,
    pub today_volume: String,
    pub total_transactions: u64,
    pub today_transactions: u64,
}

/// Summary totals across all sub-merchants.
#[derive(Debug, Serialize)]
pub struct SubMerchantStatsSummary {
    pub total_volume: String,
    pub today_volume: String,
    pub total_transactions: u64,
    pub today_transactions: u64,
}

/// Full stats response.
#[derive(Debug, Serialize)]
pub struct SubMerchantStatsResponse {
    pub summary: SubMerchantStatsSummary,
    pub sub_merchants: Vec<SubMerchantStatsEntry>,
}

/// Helper for aggregated SQL result.
#[derive(Debug, FromQueryResult)]
struct StatsRow {
    pub merchant_id: String,
    pub volume: Option<sea_orm::prelude::Decimal>,
    pub count: i64,
}

/// Service error types (mapped to HTTP errors by handlers).
#[derive(Debug, thiserror::Error)]
pub enum SubMerchantError {
    #[error("Parent org '{0}' not found")]
    ParentNotFound(String),
    #[error("Parent org '{0}' is not active")]
    ParentInactive(String),
    #[error("Sub-merchant code '{code}' already exists for parent '{parent}'")]
    DuplicateCode { parent: String, code: String },
    #[error("Sub-merchant code '{0}' is reserved")]
    ReservedCode(String),
    #[error("Sub-merchant not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl From<SubMerchantError> for crate::api::error::AppError {
    fn from(e: SubMerchantError) -> Self {
        use crate::api::error::AppError;
        match e {
            SubMerchantError::ParentNotFound(id) => {
                AppError::NotFound(format!("Parent org '{}' not found", id))
            }
            SubMerchantError::ParentInactive(id) => AppError::ValidationError {
                code: "parent_org_inactive",
                message: format!("Parent org '{}' is not active", id),
                param: Some("parent_org_id".into()),
            },
            SubMerchantError::DuplicateCode { parent, code } => AppError::Conflict(format!(
                "Sub-merchant code '{}' already exists for parent '{}'",
                code, parent
            )),
            SubMerchantError::ReservedCode(code) => AppError::ValidationError {
                code: "reserved_code",
                message: format!(
                    "Sub-merchant code '{}' is reserved and cannot be used",
                    code
                ),
                param: Some("sub_merchant_code".into()),
            },
            SubMerchantError::NotFound => AppError::NotFound("Sub-merchant not found".into()),
            SubMerchantError::Database(e) => AppError::InternalServerError(e.into()),
            SubMerchantError::Internal(e) => AppError::InternalServerError(e),
        }
    }
}

pub struct SubMerchantService {
    db: DatabaseConnection,
    address_manager: Arc<AddressManager>,
    enabled_networks: Vec<Network>,
    config: Arc<Config>,
}

impl SubMerchantService {
    pub fn new(
        db: DatabaseConnection,
        address_manager: Arc<AddressManager>,
        enabled_networks: Vec<Network>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            db,
            address_manager,
            enabled_networks,
            config,
        }
    }

    /// Create a new sub-merchant under a parent org.
    ///
    /// 1. Validates parent org exists & is active
    /// 2. Checks for duplicate code
    /// 3. Creates hidden merchant org (merchant_type = sub_merchant)
    /// 4. Creates sub_merchants mapping record
    /// 5. Initializes HD addresses (idempotent, non-fatal on failure)
    pub async fn create(
        &self,
        input: CreateSubMerchantInput,
    ) -> Result<SubMerchantResponse, SubMerchantError> {
        // 1. Validate parent org
        let parent = Merchants::find_by_id(&input.parent_org_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| SubMerchantError::ParentNotFound(input.parent_org_id.clone()))?;

        if parent.status != merchants::MerchantStatus::Active {
            return Err(SubMerchantError::ParentInactive(
                input.parent_org_id.clone(),
            ));
        }

        // 2. Validate reserved codes (used as filter keywords)
        if input.sub_merchant_code.starts_with('_') {
            return Err(SubMerchantError::ReservedCode(
                input.sub_merchant_code.clone(),
            ));
        }

        // 3. Check duplicate code
        let existing = SubMerchants::find()
            .filter(sub_merchants::Column::ParentOrgId.eq(&input.parent_org_id))
            .filter(sub_merchants::Column::SubMerchantCode.eq(&input.sub_merchant_code))
            .one(&self.db)
            .await?;

        if existing.is_some() {
            return Err(SubMerchantError::DuplicateCode {
                parent: input.parent_org_id,
                code: input.sub_merchant_code,
            });
        }

        // 3. Begin transaction — create merchant org + mapping record
        let txn = self.db.begin().await?;

        let child_org_id = format!("mer_{}", Uuid::new_v4().to_string().replace('-', ""));
        let now = Utc::now();

        let child_org = merchants::ActiveModel {
            id: Set(child_org_id.clone()),
            name: Set(format!(
                "[Sub] {} ({})",
                input.display_name, input.sub_merchant_code
            )),
            status: Set(merchants::MerchantStatus::Active),
            merchant_type: Set(merchants::MerchantType::SubMerchant),
            custom_fee_percentage: Set(parent.custom_fee_percentage),
            fee_tier: Set(parent.fee_tier.clone()),
            fee_source: Set(parent.fee_source.clone()),
            first_month_ends_at: Set(parent.first_month_ends_at),
            last_month_volume: Set(0),
            tier_updated_at: Set(now.into()),
            referred_by_agent_id: Set(parent.referred_by_agent_id.clone()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            ..Default::default()
        };
        child_org.insert(&txn).await?;

        let sm_id = format!("sm_{}", Uuid::new_v4().to_string().replace('-', ""));
        let sm_record = sub_merchants::ActiveModel {
            id: Set(sm_id.clone()),
            parent_org_id: Set(input.parent_org_id.clone()),
            sub_merchant_code: Set(input.sub_merchant_code.clone()),
            display_name: Set(input.display_name.clone()),
            child_org_id: Set(child_org_id.clone()),
            status: Set(sub_merchants::SubMerchantStatus::Active),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };
        let sm_model = sm_record.insert(&txn).await?;

        txn.commit().await?;

        // 4. Initialize HD addresses (outside txn, idempotent)
        let env = self.config.environment.to_entity_environment();
        for network in &self.enabled_networks {
            match self
                .address_manager
                .initialize_merchant_addresses(&child_org_id, *network, env.clone())
                .await
            {
                Ok(result) => {
                    info!(
                        child_org_id = %child_org_id,
                        network = ?network,
                        addresses_created = result.addresses_created,
                        "Initialized HD addresses for sub-merchant"
                    );
                }
                Err(e) => {
                    warn!(
                        child_org_id = %child_org_id,
                        network = ?network,
                        error = %e,
                        "Failed to pre-initialize addresses for sub-merchant (will retry lazily)"
                    );
                }
            }
        }

        info!(
            sub_merchant_id = %sm_id,
            parent_org_id = %input.parent_org_id,
            sub_merchant_code = %input.sub_merchant_code,
            child_org_id = %child_org_id,
            "Sub-merchant created"
        );

        Ok(sm_model.into())
    }

    /// List sub-merchants. If parent_org_id is provided, filter to that parent only.
    pub async fn list(
        &self,
        parent_org_id: Option<&str>,
        pagination: Pagination,
    ) -> Result<PaginatedResult<SubMerchantResponse>, SubMerchantError> {
        let mut query = SubMerchants::find();

        if let Some(parent_id) = parent_org_id {
            query = query.filter(sub_merchants::Column::ParentOrgId.eq(parent_id));
        }

        let total = query.clone().count(&self.db).await?;
        let offset = (pagination.page - 1) * pagination.page_size;
        let items: Vec<sub_merchants::Model> = query
            .order_by_desc(sub_merchants::Column::CreatedAt)
            .offset(offset)
            .limit(pagination.page_size)
            .all(&self.db)
            .await?;

        Ok(PaginatedResult {
            items: items.into_iter().map(SubMerchantResponse::from).collect(),
            total,
            page: pagination.page,
            page_size: pagination.page_size,
        })
    }

    /// Get a sub-merchant by its internal ID (sm_xxx). Used by admin API.
    pub async fn get_by_id(&self, id: &str) -> Result<SubMerchantResponse, SubMerchantError> {
        let sm = SubMerchants::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or(SubMerchantError::NotFound)?;
        Ok(sm.into())
    }

    /// Get a sub-merchant by parent org + code. Used by public/internal API.
    pub async fn get_by_code(
        &self,
        parent_org_id: &str,
        code: &str,
    ) -> Result<SubMerchantResponse, SubMerchantError> {
        let sm = SubMerchants::find()
            .filter(sub_merchants::Column::ParentOrgId.eq(parent_org_id))
            .filter(sub_merchants::Column::SubMerchantCode.eq(code))
            .one(&self.db)
            .await?
            .ok_or(SubMerchantError::NotFound)?;
        Ok(sm.into())
    }

    /// Update a sub-merchant (by internal ID). Used by admin API.
    pub async fn update_by_id(
        &self,
        id: &str,
        input: UpdateSubMerchantInput,
    ) -> Result<SubMerchantResponse, SubMerchantError> {
        let sm = SubMerchants::find_by_id(id)
            .one(&self.db)
            .await?
            .ok_or(SubMerchantError::NotFound)?;

        let child_org_id = sm.child_org_id.clone();
        let mut active: sub_merchants::ActiveModel = sm.into();
        let now = Utc::now();

        if let Some(name) = input.display_name {
            active.display_name = Set(name);
        }
        if let Some(ref status) = input.status {
            active.status = Set(status.clone());
        }
        active.updated_at = Set(now.into());

        let updated = active.update(&self.db).await?;

        // Cascade status change to child merchant org
        if let Some(status) = input.status {
            self.cascade_status_to_child(&child_org_id, &status, now)
                .await?;
        }

        Ok(updated.into())
    }

    /// Update a sub-merchant (by parent org + code). Used by public/internal API.
    /// Ensures IDOR protection: only the owning parent can update.
    pub async fn update_by_code(
        &self,
        parent_org_id: &str,
        code: &str,
        input: UpdateSubMerchantInput,
    ) -> Result<SubMerchantResponse, SubMerchantError> {
        let sm = SubMerchants::find()
            .filter(sub_merchants::Column::ParentOrgId.eq(parent_org_id))
            .filter(sub_merchants::Column::SubMerchantCode.eq(code))
            .one(&self.db)
            .await?
            .ok_or(SubMerchantError::NotFound)?;

        let child_org_id = sm.child_org_id.clone();
        let mut active: sub_merchants::ActiveModel = sm.into();
        let now = Utc::now();

        if let Some(name) = input.display_name {
            active.display_name = Set(name);
        }
        if let Some(ref status) = input.status {
            active.status = Set(status.clone());
        }
        active.updated_at = Set(now.into());

        let updated = active.update(&self.db).await?;

        // Cascade status change to child merchant org
        if let Some(status) = input.status {
            self.cascade_status_to_child(&child_org_id, &status, now)
                .await?;
        }

        Ok(updated.into())
    }

    /// Cascade sub-merchant status change to the child merchant org.
    /// Maps SubMerchantStatus → MerchantStatus (Active↔Active, Suspended↔Suspended).
    async fn cascade_status_to_child(
        &self,
        child_org_id: &str,
        status: &sub_merchants::SubMerchantStatus,
        now: chrono::DateTime<Utc>,
    ) -> Result<(), SubMerchantError> {
        let merchant_status = match status {
            sub_merchants::SubMerchantStatus::Active => merchants::MerchantStatus::Active,
            sub_merchants::SubMerchantStatus::Suspended => merchants::MerchantStatus::Suspended,
        };

        let child = Merchants::find_by_id(child_org_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                SubMerchantError::Internal(anyhow::anyhow!(
                    "Child org '{}' not found during status cascade",
                    child_org_id
                ))
            })?;

        if child.status != merchant_status {
            let mut child_active: merchants::ActiveModel = child.into();
            child_active.status = Set(merchant_status.clone());
            child_active.updated_at = Set(now.into());
            child_active.update(&self.db).await?;

            info!(
                child_org_id = %child_org_id,
                new_status = ?merchant_status,
                "Cascaded sub-merchant status to child org"
            );
        }

        Ok(())
    }

    // =========================================================================
    // Shared Helpers (used by mixed-display across Sessions/Resolution/etc.)
    // =========================================================================

    /// Get all child_org_ids for a parent (regardless of status).
    /// Used to build `WHERE merchant_id IN (...)` queries for mixed display.
    pub async fn get_all_child_org_ids(
        &self,
        parent_org_id: &str,
    ) -> Result<Vec<String>, SubMerchantError> {
        let ids: Vec<String> = SubMerchants::find()
            .filter(sub_merchants::Column::ParentOrgId.eq(parent_org_id))
            .select_only()
            .column(sub_merchants::Column::ChildOrgId)
            .into_tuple::<String>()
            .all(&self.db)
            .await?;
        Ok(ids)
    }

    /// Build child_org_id → sub_merchant_code mapping for batch reverse lookup.
    /// Returns empty map if parent has no sub-merchants.
    pub async fn get_code_map(
        &self,
        parent_org_id: &str,
    ) -> Result<HashMap<String, String>, SubMerchantError> {
        let rows: Vec<(String, String)> = SubMerchants::find()
            .filter(sub_merchants::Column::ParentOrgId.eq(parent_org_id))
            .select_only()
            .column(sub_merchants::Column::ChildOrgId)
            .column(sub_merchants::Column::SubMerchantCode)
            .into_tuple::<(String, String)>()
            .all(&self.db)
            .await?;
        Ok(rows.into_iter().collect())
    }

    /// Resolve a sub_merchant_code filter to a list of merchant_ids to query.
    ///
    /// Returns `(merchant_ids, code_map)` where:
    /// - `merchant_ids`: the IDs to use in `WHERE merchant_id IN (...)`
    /// - `code_map`: child_org_id → sub_merchant_code (for DTO enrichment)
    ///
    /// Filter logic:
    /// - `include_sub_merchants=false` (or absent) → `[parent_id]`, empty map
    /// - `include_sub_merchants=true`, no specific code → `[parent_id, child_1, child_2, ...]`
    /// - `sub_merchant_code=_self` → `[parent_id]`
    /// - `sub_merchant_code=xxx` → `[child_org_id_for_xxx]`
    pub async fn resolve_merchant_ids(
        &self,
        parent_org_id: &str,
        include_sub_merchants: bool,
        sub_merchant_code: Option<&str>,
    ) -> Result<(Vec<String>, HashMap<String, String>), SubMerchantError> {
        // Short-circuit: no sub-merchant inclusion requested
        if !include_sub_merchants && sub_merchant_code.is_none() {
            return Ok((vec![parent_org_id.to_string()], HashMap::new()));
        }

        // If specific code is "_self", return only parent
        if sub_merchant_code == Some("_self") {
            return Ok((vec![parent_org_id.to_string()], HashMap::new()));
        }

        // Load all sub-merchant mappings
        let code_map = self.get_code_map(parent_org_id).await?;

        if code_map.is_empty() {
            // No sub-merchants at all → just parent
            return Ok((vec![parent_org_id.to_string()], HashMap::new()));
        }

        // If specific code requested, find its child_org_id
        if let Some(code) = sub_merchant_code {
            let child_id = code_map
                .iter()
                .find(|(_, c)| c.as_str() == code)
                .map(|(id, _)| id.clone())
                .ok_or(SubMerchantError::NotFound)?;
            return Ok((vec![child_id], code_map));
        }

        // include_sub_merchants=true, no specific code → parent + all children
        let mut ids: Vec<String> = vec![parent_org_id.to_string()];
        ids.extend(code_map.keys().cloned());
        Ok((ids, code_map))
    }

    /// Get aggregated transaction stats for all sub-merchants under a parent org.
    ///
    /// Returns per-sub-merchant volume/count (total + today) plus summary totals.
    pub async fn get_stats(
        &self,
        parent_org_id: &str,
    ) -> Result<SubMerchantStatsResponse, SubMerchantError> {
        // 1. Load all sub-merchants for the parent
        let subs: Vec<sub_merchants::Model> = SubMerchants::find()
            .filter(sub_merchants::Column::ParentOrgId.eq(parent_org_id))
            .order_by_asc(sub_merchants::Column::SubMerchantCode)
            .all(&self.db)
            .await?;

        if subs.is_empty() {
            return Ok(SubMerchantStatsResponse {
                summary: SubMerchantStatsSummary {
                    total_volume: "0".to_string(),
                    today_volume: "0".to_string(),
                    total_transactions: 0,
                    today_transactions: 0,
                },
                sub_merchants: vec![],
            });
        }

        // Build child_org_id → sub_merchant mapping
        let child_ids: Vec<String> = subs.iter().map(|s| s.child_org_id.clone()).collect();

        let today_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        // 2. Total stats grouped by merchant_id
        let total_rows: Vec<StatsRow> = Transactions::find()
            .filter(transactions::Column::MerchantId.is_in(&child_ids))
            .select_only()
            .column(transactions::Column::MerchantId)
            .column_as(transactions::Column::Amount.sum(), "volume")
            .column_as(transactions::Column::TxHash.count(), "count")
            .group_by(transactions::Column::MerchantId)
            .into_model::<StatsRow>()
            .all(&self.db)
            .await?;

        // 3. Today stats grouped by merchant_id
        let today_rows: Vec<StatsRow> = Transactions::find()
            .filter(transactions::Column::MerchantId.is_in(&child_ids))
            .filter(transactions::Column::CreatedAt.gte(today_start))
            .select_only()
            .column(transactions::Column::MerchantId)
            .column_as(transactions::Column::Amount.sum(), "volume")
            .column_as(transactions::Column::TxHash.count(), "count")
            .group_by(transactions::Column::MerchantId)
            .into_model::<StatsRow>()
            .all(&self.db)
            .await?;

        // Build lookup maps: child_org_id → (volume, count)
        let total_map: std::collections::HashMap<String, (sea_orm::prelude::Decimal, u64)> =
            total_rows
                .into_iter()
                .map(|r| {
                    (
                        r.merchant_id,
                        (
                            r.volume.unwrap_or(sea_orm::prelude::Decimal::ZERO),
                            r.count as u64,
                        ),
                    )
                })
                .collect();

        let today_map: std::collections::HashMap<String, (sea_orm::prelude::Decimal, u64)> =
            today_rows
                .into_iter()
                .map(|r| {
                    (
                        r.merchant_id,
                        (
                            r.volume.unwrap_or(sea_orm::prelude::Decimal::ZERO),
                            r.count as u64,
                        ),
                    )
                })
                .collect();

        let divisor = rust_decimal::Decimal::from(1_000_000_i64);
        let mut summary_total_vol = sea_orm::prelude::Decimal::ZERO;
        let mut summary_today_vol = sea_orm::prelude::Decimal::ZERO;
        let mut summary_total_count: u64 = 0;
        let mut summary_today_count: u64 = 0;

        // 4. Build per-sub-merchant entries
        let entries: Vec<SubMerchantStatsEntry> = subs
            .into_iter()
            .map(|sm| {
                let (tv, tc) = total_map
                    .get(&sm.child_org_id)
                    .cloned()
                    .unwrap_or((sea_orm::prelude::Decimal::ZERO, 0));
                let (dv, dc) = today_map
                    .get(&sm.child_org_id)
                    .cloned()
                    .unwrap_or((sea_orm::prelude::Decimal::ZERO, 0));

                summary_total_vol += tv;
                summary_today_vol += dv;
                summary_total_count += tc;
                summary_today_count += dc;

                SubMerchantStatsEntry {
                    sub_merchant_code: sm.sub_merchant_code,
                    display_name: sm.display_name,
                    status: sm.status,
                    total_volume: (tv / divisor).normalize().to_string(),
                    today_volume: (dv / divisor).normalize().to_string(),
                    total_transactions: tc,
                    today_transactions: dc,
                }
            })
            .collect();

        Ok(SubMerchantStatsResponse {
            summary: SubMerchantStatsSummary {
                total_volume: (summary_total_vol / divisor).normalize().to_string(),
                today_volume: (summary_today_vol / divisor).normalize().to_string(),
                total_transactions: summary_total_count,
                today_transactions: summary_today_count,
            },
            sub_merchants: entries,
        })
    }
}
