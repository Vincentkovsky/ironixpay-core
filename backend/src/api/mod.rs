pub mod docs;
pub mod dtos;
pub mod error;
pub mod middleware;
pub mod routes;

use axum::{middleware as axum_mw, Router};

pub fn create_router() -> Router<crate::AppState> {
    // Internal dashboard API — JWT-only auth, applied once at the group level.
    // Individual modules (webhooks, resolution, billing) no longer need their own auth layer.
    let internal = Router::new()
        .nest("/merchants", routes::merchants::internal_router())
        .nest("/sub-merchants", routes::sub_merchants::internal_router())
        .nest("/webhooks", routes::webhooks::router())
        .nest("/resolution", routes::resolution::router())
        .nest("/billing", routes::billing::router())
        .nest("/config", routes::config::router())
        .nest("/team", routes::team::router())
        .nest("/agent", routes::agent::router())
        .nest("/analytics", routes::analytics::router())
        .nest("/branding", routes::branding::router())
        .nest("/xero", routes::xero::router())
        .route_layer(axum_mw::from_fn(middleware::auth::jwt_auth));

    // Admin portal API — ADMIN_TOKEN auth, platform-wide access.
    let admin = Router::new()
        .nest("/admin", routes::admin::router())
        .route_layer(axum_mw::from_fn(middleware::admin_auth::admin_auth));

    Router::new()
        // Health check routes (no auth, for probes/monitoring)
        .merge(routes::health::router())
        // Public checkout API (self-manages auth internally: public + unified auth)
        .nest("/v1/checkout", routes::checkout::router())
        // Public payout API (API Key auth, same as checkout)
        .nest("/v1/payouts", routes::payouts::router())
        // Public sub-merchant management API (API Key auth, NO sub_merchant_scope)
        .nest("/v1/sub-merchants", routes::sub_merchants::router())
        // Auth routes (no auth required)
        .nest("/api/auth", routes::merchants::auth_router())
        // Public website lead intake (validated, body-limited, globally IP rate-limited)
        .nest("/api/public", routes::leads::router())
        // Internal dashboard routes (JWT auth)
        .nest("/api/internal", internal)
        // Admin portal routes (ADMIN_TOKEN auth)
        .nest("/api", admin)
        // Helius webhook (validates its own Authorization header, no JWT/API Key)
        .nest("/webhooks/helius", routes::helius_webhook::router())
        // API documentation (Scalar via CDN)
        .merge(docs::router())
}
