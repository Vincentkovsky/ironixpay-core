use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BusinessType {
    Ecommerce,
    SaasDigital,
    ForexFinancial,
    PspMarketplace,
    Other,
}

impl BusinessType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ecommerce => "ecommerce",
            Self::SaasDigital => "saas_digital",
            Self::ForexFinancial => "forex_financial",
            Self::PspMarketplace => "psp_marketplace",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MonthlyVolume {
    #[serde(rename = "under_50k")]
    Under50k,
    #[serde(rename = "50k_250k")]
    From50kTo250k,
    #[serde(rename = "250k_1m")]
    From250kTo1m,
    #[serde(rename = "above_1m")]
    Above1m,
    NotSure,
}

impl MonthlyVolume {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Under50k => "under_50k",
            Self::From50kTo250k => "50k_250k",
            Self::From250kTo1m => "250k_1m",
            Self::Above1m => "above_1m",
            Self::NotSure => "not_sure",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPreference {
    Tron,
    Solana,
    Ethereum,
    Bsc,
    L2,
    NotSure,
}

impl NetworkPreference {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tron => "tron",
            Self::Solana => "solana",
            Self::Ethereum => "ethereum",
            Self::Bsc => "bsc",
            Self::L2 => "l2",
            Self::NotSure => "not_sure",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationNeed {
    Checkout,
    PaymentApi,
    Payouts,
    SubMerchants,
    Other,
}

impl IntegrationNeed {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Checkout => "checkout",
            Self::PaymentApi => "payment_api",
            Self::Payouts => "payouts",
            Self::SubMerchants => "sub_merchants",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct EnterpriseLeadRequest {
    #[validate(length(min = 2, max = 120, message = "Company name must be 2-120 characters"))]
    pub company_name: String,

    #[validate(
        url(message = "Company website must be a valid URL"),
        length(max = 300)
    )]
    pub company_website: Option<String>,

    #[validate(email(message = "Invalid work email format"), length(max = 254))]
    pub contact_email: String,

    #[validate(length(max = 100, message = "Telegram contact is too long"))]
    pub telegram: Option<String>,

    pub business_type: BusinessType,
    pub monthly_volume: MonthlyVolume,

    #[validate(length(min = 1, max = 6, message = "Select at least one network"))]
    pub networks: Vec<NetworkPreference>,

    #[validate(length(min = 1, max = 5, message = "Select at least one integration need"))]
    pub integration_needs: Vec<IntegrationNeed>,

    #[validate(length(max = 1000, message = "Additional context is too long"))]
    pub message: Option<String>,

    #[validate(length(min = 2, max = 2, message = "Locale must be en or zh"))]
    pub locale: String,

    /// Honeypot field. Real users never see or populate it.
    #[serde(default)]
    #[validate(length(max = 200))]
    pub fax_number: String,
}

impl EnterpriseLeadRequest {
    pub fn normalize(&mut self) {
        self.company_name = self.company_name.trim().to_string();
        self.company_website = normalize_optional(self.company_website.take());
        self.contact_email = self.contact_email.trim().to_lowercase();
        self.telegram = normalize_optional(self.telegram.take());
        self.message = normalize_optional(self.message.take());
        self.locale = self.locale.trim().to_lowercase();
        self.fax_number = self.fax_number.trim().to_string();
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[derive(Debug, Serialize)]
pub struct EnterpriseLeadResponse {
    pub accepted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_request() -> EnterpriseLeadRequest {
        EnterpriseLeadRequest {
            company_name: "Acme Payments".to_string(),
            company_website: Some("https://acme.example".to_string()),
            contact_email: "ops@acme.example".to_string(),
            telegram: Some("@acme_ops".to_string()),
            business_type: BusinessType::PspMarketplace,
            monthly_volume: MonthlyVolume::From250kTo1m,
            networks: vec![NetworkPreference::Tron, NetworkPreference::L2],
            integration_needs: vec![IntegrationNeed::PaymentApi],
            message: None,
            locale: "en".to_string(),
            fax_number: String::new(),
        }
    }

    #[test]
    fn accepts_valid_enterprise_lead() {
        assert!(valid_request().validate().is_ok());
    }

    #[test]
    fn rejects_missing_networks() {
        let mut request = valid_request();
        request.networks.clear();
        assert!(request.validate().is_err());
    }

    #[test]
    fn normalizes_contact_fields() {
        let mut request = valid_request();
        request.company_name = "  Acme Payments  ".to_string();
        request.contact_email = "  OPS@ACME.EXAMPLE ".to_string();
        request.telegram = Some("   ".to_string());
        request.normalize();

        assert_eq!(request.company_name, "Acme Payments");
        assert_eq!(request.contact_email, "ops@acme.example");
        assert_eq!(request.telegram, None);
    }

    #[test]
    fn deserializes_frontend_wire_values() {
        let request: EnterpriseLeadRequest = serde_json::from_value(serde_json::json!({
            "company_name": "Acme Payments",
            "company_website": null,
            "contact_email": "ops@acme.example",
            "telegram": null,
            "business_type": "psp_marketplace",
            "monthly_volume": "50k_250k",
            "networks": ["tron", "l2"],
            "integration_needs": ["payment_api", "payouts"],
            "message": null,
            "locale": "en",
            "fax_number": ""
        }))
        .expect("frontend payload should deserialize");

        assert_eq!(request.monthly_volume.as_str(), "50k_250k");
        assert_eq!(request.networks[1].as_str(), "l2");
    }
}
