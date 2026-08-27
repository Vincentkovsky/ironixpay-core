//! Public enterprise lead intake route.

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use tower_http::limit::RequestBodyLimitLayer;
use validator::Validate;

use crate::api::dtos::leads::{EnterpriseLeadRequest, EnterpriseLeadResponse};
use crate::api::error::{AppError, AppJson, E_PARAMETER_INVALID};
use crate::services::lead::LeadSubmission;
use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/enterprise-leads", post(create_enterprise_lead))
        .layer(RequestBodyLimitLayer::new(16 * 1024))
}

async fn create_enterprise_lead(
    State(state): State<AppState>,
    AppJson(mut body): AppJson<EnterpriseLeadRequest>,
) -> Result<(StatusCode, Json<EnterpriseLeadResponse>), AppError> {
    body.normalize();

    // Return a normal response to automated form fillers without storing or emailing.
    if !body.fax_number.is_empty() {
        return Ok((
            StatusCode::ACCEPTED,
            Json(EnterpriseLeadResponse { accepted: true }),
        ));
    }

    body.validate()?;
    if body.locale != "en" && body.locale != "zh" {
        return Err(AppError::ValidationError {
            code: E_PARAMETER_INVALID,
            message: "Locale must be en or zh".to_string(),
            param: Some("locale".to_string()),
        });
    }

    let submission = LeadSubmission {
        company_name: body.company_name,
        company_website: body.company_website,
        contact_email: body.contact_email,
        telegram: body.telegram,
        business_type: body.business_type.as_str().to_string(),
        monthly_volume: body.monthly_volume.as_str().to_string(),
        networks: body
            .networks
            .iter()
            .map(|network| network.as_str().to_string())
            .collect(),
        integration_needs: body
            .integration_needs
            .iter()
            .map(|need| need.as_str().to_string())
            .collect(),
        message: body.message,
        locale: body.locale,
    };

    let lead = state.lead_service.create(submission).await?;
    let lead_service = state.lead_service.clone();
    tokio::spawn(async move {
        lead_service.notify(lead).await;
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(EnterpriseLeadResponse { accepted: true }),
    ))
}
