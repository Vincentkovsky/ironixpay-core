//! OpenAPI documentation configuration
//!
//! Serves the OpenAPI spec as JSON and an interactive Scalar UI.
//! Uses Scalar via CDN to avoid axum version conflicts with utoipa-scalar.

use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::AppState;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "IronixPay API",
        version = "1.0.0",
        description = include_str!("../../docs/api_intro.md"),
        contact(name = "IronixPay", url = "https://ironixpay.com"),
    ),
    servers(
        (url = "https://sandbox.ironixpay.com", description = "Sandbox (Testnet)"),
        (url = "https://api.ironixpay.com", description = "Production (Mainnet)"),
    ),
    tags(
        (name = "Checkout Sessions", description = "Create and retrieve payment sessions"),
        (name = "Payouts", description = "Send USDT from merchant balance to any on-chain address"),
        (name = "Sub-Merchants", description = "Manage sub-merchants for PSP/marketplace use cases"),
    ),
    paths(
        crate::api::routes::checkout::create_session,
        crate::api::routes::checkout::get_session,
        crate::api::routes::checkout::list_sessions,
        crate::api::routes::payouts::create_payout,
        crate::api::routes::payouts::get_payout,
        crate::api::routes::payouts::list_payouts,
        crate::api::routes::sub_merchants::create,
        crate::api::routes::sub_merchants::list,
        crate::api::routes::sub_merchants::get_by_code,
        crate::api::routes::sub_merchants::update,
    ),
    components(schemas(
        crate::api::dtos::checkout::CreateSessionBody,
        crate::api::dtos::checkout::SessionResponse,
        crate::api::dtos::checkout::TransactionResponse,
        crate::api::dtos::payouts::CreatePayoutBody,
        crate::api::dtos::payouts::PayoutResponse,
        crate::services::sub_merchant::SubMerchantResponse,
        crate::api::routes::sub_merchants::CreateBody,
        crate::api::routes::sub_merchants::UpdateBody,
        crate::entity::checkout_sessions::SessionStatus,
        crate::entity::payouts::PayoutStatus,
        crate::entity::sub_merchants::SubMerchantStatus,
        crate::entity::network::Network,
        crate::api::error::ApiErrorResponse,
        crate::api::error::ApiErrorBody,
        crate::api::error::ApiErrorType,
    )),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .description(Some(
                            "API key authentication. Use `sk_test_...` for sandbox or `sk_live_...` for production.",
                        ))
                        .build(),
                ),
            );
        }
    }
}

/// Router for API documentation endpoints.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/docs", get(scalar_html))
        .route("/docs/openapi.json", get(openapi_json))
        .route("/brand/favicon.svg", get(favicon_svg))
}

/// IronixPay brand favicon SVG (shared with frontend apps)
const FAVICON_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 32 32\">\
<rect width=\"32\" height=\"32\" rx=\"6\" fill=\"rgb(37,99,235)\"/>\
<text x=\"16\" y=\"23\" text-anchor=\"middle\" \
font-family=\"Inter, system-ui, sans-serif\" \
font-size=\"20\" font-weight=\"700\" fill=\"white\">IX</text>\
</svg>";

/// Serves the OpenAPI spec as JSON.
async fn openapi_json() -> impl IntoResponse {
    Json(ApiDoc::openapi())
}

/// Serves the brand favicon as SVG.
async fn favicon_svg() -> impl IntoResponse {
    (
        [
            ("content-type", "image/svg+xml"),
            ("cache-control", "public, max-age=86400"),
        ],
        FAVICON_SVG,
    )
}

/// Serves the Scalar UI via CDN.
async fn scalar_html() -> impl IntoResponse {
    Html(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>IronixPay API Reference</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <link rel="icon" type="image/svg+xml" href="/brand/favicon.svg" />
</head>
<body>
    <script id="api-reference" data-url="/docs/openapi.json"></script>
    <script>
      document.getElementById('api-reference').dataset.configuration = JSON.stringify({
        theme: 'kepler',
        favicon: '/brand/favicon.svg',
        metaData: {
          title: 'IronixPay API Reference',
        },
      })
    </script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
</body>
</html>"#,
    )
}
