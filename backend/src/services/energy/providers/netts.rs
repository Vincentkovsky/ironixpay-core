use super::super::interface::{EnergyReceipt, EnergyRentalProvider};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::{header, Client, Url};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;
use tracing::{error, info, warn};

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

#[derive(Clone)]
pub struct NettsEnergyProvider {
    client: Client,
    base_url: Url,
}

impl fmt::Debug for NettsEnergyProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NettsEnergyProvider")
            .field("base_url", &self.base_url)
            .field("api_key", &"***") // Mask sensitive data
            .finish()
    }
}

impl NettsEnergyProvider {
    pub fn try_new(api_key: String, base_url_str: String) -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        let mut api_key_val =
            header::HeaderValue::from_str(&api_key).context("Invalid API Key format")?;
        api_key_val.set_sensitive(true);
        headers.insert("X-API-KEY", api_key_val);

        // No retry middleware for potentially non-idempotent order requests
        // Use default headers for efficiency and cleanliness
        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(10))
            .build()
            .context("Failed to build HTTP client for Netts provider")?;

        let mut base_url = Url::parse(&base_url_str)
            .map_err(|e| anyhow!("Invalid Netts base URL '{}': {}", base_url_str, e))?;

        // Ensure base_url ends with a slash to prevent `join` from stripping the last path segment
        if !base_url.path().ends_with('/') {
            base_url
                .path_segments_mut()
                .map_err(|_| anyhow!("Base URL cannot be a base"))?
                .pop_if_empty()
                .push("");
        }

        Ok(Self { client, base_url })
    }
}

#[derive(Debug, Serialize)]
struct OrderRequest {
    amount: u64,
    #[serde(rename = "receiveAddress")]
    receive_address: String,
}

#[derive(Debug, Deserialize)]
struct OrderResponse {
    detail: OrderDetail,
}

#[derive(Debug, Deserialize)]
struct OrderDetail {
    code: i32,
    msg: String,
    data: Option<OrderData>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // API response fields
struct OrderData {
    #[serde(rename = "orderId")]
    order_id: String,
    #[serde(rename = "paidTRX")]
    paid_trx: Decimal,
    hash: String,
    #[serde(rename = "energy")]
    energy_params: u64,
    #[serde(rename = "delegateAddress")]
    delegate_address: String,
}

#[async_trait]
impl EnergyRentalProvider for NettsEnergyProvider {
    async fn delegate_energy(
        &self,
        target_address: &str,
        energy_amount: u64,
    ) -> Result<EnergyReceipt> {
        let url = self
            .base_url
            .join("order1h")
            .context("Failed to join URL path for order1h")?;

        let payload = OrderRequest {
            amount: energy_amount,
            receive_address: target_address.to_string(),
        };

        info!(
            "Delegating {} energy to {} (1h default) via Netts",
            energy_amount, target_address
        );

        let response = self.client.post(url).json(&payload).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!(%status, %error_text, "Netts API error");
            return Err(anyhow!("Netts API error ({}): {}", status, error_text));
        }

        let text = response.text().await?;
        let order_res: OrderResponse = serde_json::from_str(&text).map_err(|e| {
            let snippet: String = text.chars().take(200).collect();
            anyhow!(
                "Failed to parse Netts response: {}. Body snippet: '{}'",
                e,
                snippet
            )
        })?;

        if order_res.detail.code != 10000 {
            return Err(anyhow!(
                "Netts order failed: {} (code {})",
                order_res.detail.msg,
                order_res.detail.code
            ));
        }

        let data = order_res
            .detail
            .data
            .ok_or_else(|| anyhow!("Missing order data in Netts response"))?;

        // Netts sometimes returns code 10000 (success) but with an empty hash when the
        // delegation tx hasn't been mined yet. Since `ensure_resources` calls `wait_for_energy`
        // after this to verify on-chain, we can safely proceed with a placeholder.
        let trx_hash = if data.hash.len() == 64 {
            data.hash
        } else {
            warn!(
                hash_len = data.hash.len(),
                order_id = %data.order_id,
                "Netts returned empty/short tx hash — delegation may still be processing. \
                 Proceeding; wait_for_energy will verify on-chain."
            );
            format!("netts_pending_{}", data.order_id)
        };

        // Convert Decimal cost to sun (u64)
        let multiplier = Decimal::new(1_000_000, 0);
        let cost_sun_decimal = data.paid_trx * multiplier;

        let cost_sun = cost_sun_decimal
            .to_u64()
            .ok_or_else(|| anyhow!("Failed to convert cost to u64: overflow or invalid"))?;

        // Hardcode expiry: Now + 1h - buffer (120s)
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + 3600 - 120;

        Ok(EnergyReceipt {
            order_id: data.order_id,
            trx_hash,
            energy_amount: data.energy_params,
            expires_at,
            cost_sun: cost_sun as i64,
        })
    }
}
