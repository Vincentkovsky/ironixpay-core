//! R2 Object Storage Service
//!
//! Handles file uploads to Cloudflare R2 (S3-compatible) for merchant branding assets.
//! Uses `aws-sdk-s3` client pointed at the R2 endpoint.

use anyhow::{bail, Context, Result};
use aws_sdk_s3::primitives::ByteStream;
use secrecy::ExposeSecret;
use std::sync::Arc;

use crate::config::Config;

/// Maximum file size for logo uploads: 2 MB
const MAX_LOGO_SIZE: usize = 2 * 1024 * 1024;

/// Allowed MIME types for logo uploads (no SVG — XSS risk)
const ALLOWED_MIME_TYPES: &[&str] = &["image/png", "image/jpeg", "image/webp"];

#[derive(Clone)]
pub struct R2StorageService {
    client: aws_sdk_s3::Client,
    bucket: String,
    public_url: String,
}

impl R2StorageService {
    /// Create a new R2StorageService from app config.
    ///
    /// Returns None if R2 is not configured (all env vars optional).
    pub async fn try_new(config: &Config) -> Option<Self> {
        let endpoint = config.r2_endpoint.as_ref()?;
        let access_key_id = config.r2_access_key_id.as_ref()?;
        let secret_access_key = config.r2_secret_access_key.as_ref()?;
        let bucket = config.r2_bucket_name.as_ref()?;
        let public_url = config.r2_public_url.as_ref()?;

        let creds = aws_sdk_s3::config::Credentials::new(
            access_key_id,
            secret_access_key.expose_secret(),
            None,
            None,
            "r2-env",
        );

        let s3_config = aws_sdk_s3::config::Builder::new()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .endpoint_url(endpoint)
            .credentials_provider(creds)
            .region(aws_sdk_s3::config::Region::new("auto"))
            .force_path_style(true)
            .build();

        let client = aws_sdk_s3::Client::from_conf(s3_config);

        tracing::info!(
            bucket = %bucket,
            public_url = %public_url,
            "R2 storage service initialized"
        );

        Some(Self {
            client,
            bucket: bucket.clone(),
            public_url: public_url.trim_end_matches('/').to_string(),
        })
    }

    /// Upload a merchant logo to R2.
    ///
    /// Returns the public URL of the uploaded logo.
    /// Path format: `merchants/{merchant_id}/logo_{timestamp}.{ext}`
    pub async fn upload_logo(
        &self,
        merchant_id: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<String> {
        // Validate size
        if data.len() > MAX_LOGO_SIZE {
            bail!(
                "File too large: {} bytes (max {} bytes / 2MB)",
                data.len(),
                MAX_LOGO_SIZE
            );
        }

        // Validate MIME type
        if !ALLOWED_MIME_TYPES.contains(&content_type) {
            bail!(
                "Unsupported file type '{}'. Allowed: PNG, JPEG, WebP",
                content_type
            );
        }

        let ext = mime_to_ext(content_type);
        let timestamp = chrono::Utc::now().timestamp();
        let key = format!("merchants/{}/logo_{}.{}", merchant_id, timestamp, ext);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(ByteStream::from(data))
            .content_type(content_type)
            .cache_control("public, max-age=31536000, immutable")
            .send()
            .await
            .context("Failed to upload logo to R2")?;

        let public_url = format!("{}/{}", self.public_url, key);

        tracing::info!(
            merchant_id = %merchant_id,
            key = %key,
            url = %public_url,
            "Logo uploaded to R2"
        );

        Ok(public_url)
    }

    /// Delete a merchant's logo from R2.
    ///
    /// Extracts the R2 key from the public URL and deletes the object.
    pub async fn delete_logo(&self, logo_url: &str) -> Result<()> {
        // Extract key from public URL: https://assets.ironixpay.com/merchants/{id}/logo_xxx.png
        let key = logo_url
            .strip_prefix(&format!("{}/", self.public_url))
            .unwrap_or(logo_url);

        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .context("Failed to delete logo from R2")?;

        tracing::info!(key = %key, "Logo deleted from R2");
        Ok(())
    }
}

/// Map MIME type to file extension
fn mime_to_ext(content_type: &str) -> &'static str {
    match content_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        _ => "bin",
    }
}

/// Convenience wrapper for optional R2 storage
pub type OptionalR2 = Option<Arc<R2StorageService>>;
