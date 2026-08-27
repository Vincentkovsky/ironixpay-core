//! Agent Service
//!
//! Manages agent lifecycle and commission tracking.
//! Agents are existing merchants promoted by Admin.
//! They refer new merchants via referral codes and earn commission (fee spread).

use anyhow::{anyhow, Result};
use chrono::Utc;
use rand::Rng;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use tracing::info;
use uuid::Uuid;

use crate::entity::{agent_profiles, merchants, AgentProfiles, Merchants};

/// Commission summary for a single merchant referred by an agent
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReferredMerchantCommission {
    pub merchant_id: String,
    pub merchant_name: String,
    pub total_fee_collected: i64,
    pub ironixpay_share: i64,
    pub agent_commission: i64,
    pub transaction_count: i64,
}

/// Aggregate commission report
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommissionReport {
    pub agent_id: String,
    pub period_start: String,
    pub period_end: String,
    pub total_fee_collected: i64,
    pub total_ironixpay_share: i64,
    pub total_agent_commission: i64,
    pub total_transactions: i64,
    pub merchants: Vec<ReferredMerchantCommission>,
}

/// Agent overview for dashboard
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentOverview {
    pub is_agent: bool,
    pub agent_id: Option<String>,
    pub referral_code: Option<String>,
    pub base_rate: Option<String>,
    pub max_markup: Option<String>,
    pub default_merchant_rate: Option<String>,
    pub referred_merchant_count: u64,
    pub total_commission: i64,
}

pub struct AgentService {
    db: DatabaseConnection,
}

impl AgentService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Generate a unique referral code (8 chars, uppercase alphanumeric)
    fn generate_referral_code() -> String {
        let chars: Vec<char> = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".chars().collect();
        let mut rng = rand::thread_rng();
        (0..8)
            .map(|_| chars[rng.gen_range(0..chars.len())])
            .collect()
    }

    /// Admin: Promote a merchant to agent status
    pub async fn create_agent(
        &self,
        merchant_id: &str,
        base_rate: Option<Decimal>,
        max_markup: Option<Decimal>,
        default_merchant_rate: Option<Decimal>,
    ) -> Result<agent_profiles::Model> {
        // Verify merchant exists
        let merchant = Merchants::find_by_id(merchant_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Merchant '{}' not found", merchant_id))?;

        // Check not already an agent
        let existing = AgentProfiles::find()
            .filter(agent_profiles::Column::MerchantId.eq(merchant_id))
            .one(&self.db)
            .await?;
        if existing.is_some() {
            return Err(anyhow!("Merchant '{}' is already an agent", merchant_id));
        }

        // Generate unique referral code
        let mut referral_code = Self::generate_referral_code();
        for _ in 0..5 {
            let exists = AgentProfiles::find()
                .filter(agent_profiles::Column::ReferralCode.eq(&referral_code))
                .one(&self.db)
                .await?;
            if exists.is_none() {
                break;
            }
            referral_code = Self::generate_referral_code();
        }

        let agent = agent_profiles::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            merchant_id: Set(merchant_id.to_string()),
            referral_code: Set(referral_code.clone()),
            base_rate: Set(base_rate.unwrap_or(Decimal::new(1, 3))), // 0.1%
            max_markup: Set(max_markup.unwrap_or(Decimal::new(4, 3))), // 0.4%
            default_merchant_rate: Set(default_merchant_rate.unwrap_or(Decimal::new(4, 3))), // 0.4%
            status: Set("active".to_string()),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        };

        let result = agent.insert(&self.db).await?;

        info!(
            agent_id = %result.id,
            merchant_id = %merchant_id,
            merchant_name = %merchant.name,
            referral_code = %referral_code,
            "Agent created"
        );

        Ok(result)
    }

    /// Lookup agent by referral code (used during registration)
    pub async fn find_by_referral_code(&self, code: &str) -> Result<Option<agent_profiles::Model>> {
        let agent = AgentProfiles::find()
            .filter(agent_profiles::Column::ReferralCode.eq(code))
            .filter(agent_profiles::Column::Status.eq("active"))
            .one(&self.db)
            .await?;
        Ok(agent)
    }

    /// Admin: List all agents (paginated)
    pub async fn list_agents(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<agent_profiles::Model>, u64)> {
        let query = AgentProfiles::find().order_by_desc(agent_profiles::Column::CreatedAt);
        let total = query.clone().count(&self.db).await?;
        let offset = (page - 1) * page_size;
        let items = query.offset(offset).limit(page_size).all(&self.db).await?;
        Ok((items, total))
    }

    /// Admin: Get agent detail by ID
    pub async fn get_agent(&self, agent_id: &str) -> Result<Option<agent_profiles::Model>> {
        let agent = AgentProfiles::find_by_id(agent_id).one(&self.db).await?;
        Ok(agent)
    }

    /// Find agent profile by merchant_id (for dashboard "am I an agent?" check)
    pub async fn find_by_merchant_id(
        &self,
        merchant_id: &str,
    ) -> Result<Option<agent_profiles::Model>> {
        let agent = AgentProfiles::find()
            .filter(agent_profiles::Column::MerchantId.eq(merchant_id))
            .one(&self.db)
            .await?;
        Ok(agent)
    }

    /// Admin: Update agent configuration
    pub async fn update_agent(
        &self,
        agent_id: &str,
        base_rate: Option<Decimal>,
        max_markup: Option<Decimal>,
        default_merchant_rate: Option<Decimal>,
        status: Option<String>,
    ) -> Result<agent_profiles::Model> {
        let agent = AgentProfiles::find_by_id(agent_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Agent '{}' not found", agent_id))?;

        let mut active: agent_profiles::ActiveModel = agent.into();

        if let Some(rate) = base_rate {
            active.base_rate = Set(rate);
        }
        if let Some(markup) = max_markup {
            active.max_markup = Set(markup);
        }
        if let Some(rate) = default_merchant_rate {
            active.default_merchant_rate = Set(rate);
        }
        if let Some(s) = status {
            active.status = Set(s);
        }

        active.updated_at = Set(Utc::now().into());
        let result = active.update(&self.db).await?;

        info!(agent_id = %agent_id, "Agent config updated");
        Ok(result)
    }

    /// Count merchants referred by a given agent
    pub async fn count_referred_merchants(&self, agent_id: &str) -> Result<u64> {
        let count = Merchants::find()
            .filter(merchants::Column::ReferredByAgentId.eq(agent_id))
            .count(&self.db)
            .await?;
        Ok(count)
    }

    /// List merchants referred by a given agent (aggregated info only)
    pub async fn list_referred_merchants(&self, agent_id: &str) -> Result<Vec<merchants::Model>> {
        let merchants = Merchants::find()
            .filter(merchants::Column::ReferredByAgentId.eq(agent_id))
            .order_by_desc(merchants::Column::CreatedAt)
            .all(&self.db)
            .await?;
        Ok(merchants)
    }

    /// Get commission report for an agent over a date range.
    ///
    /// Calculates commission from billing_logs by aggregating fee_amount
    /// and computing the agent's share (fee_amount - gross_amount * base_rate).
    pub async fn get_commission_report(
        &self,
        agent_id: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<CommissionReport> {
        use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

        let agent = AgentProfiles::find_by_id(agent_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow!("Agent '{}' not found", agent_id))?;

        // Raw SQL for commission aggregation — joins billing_logs with merchants
        // Platform share = max(gross × base_rate, 1 USDT) to cover on-chain costs.
        // Agent commission = max(0, fee − platform_share).
        let sql = r#"
            SELECT
                bl.merchant_id,
                m.name as merchant_name,
                COALESCE(SUM(
                    CASE WHEN bl.fee_amount > GREATEST(
                        (bl.gross_amount::numeric * $4::numeric)::bigint,
                        1000000
                    ) THEN bl.fee_amount ELSE 0 END
                ), 0)::bigint as total_fee,
                COALESCE(SUM(
                    CASE WHEN bl.fee_amount > GREATEST(
                        (bl.gross_amount::numeric * $4::numeric)::bigint,
                        1000000
                    ) THEN GREATEST(
                        (bl.gross_amount::numeric * $4::numeric)::bigint,
                        1000000
                    ) ELSE 0 END
                ), 0)::bigint as platform_share,
                COALESCE(SUM(
                    GREATEST(0,
                        bl.fee_amount - GREATEST(
                            (bl.gross_amount::numeric * $4::numeric)::bigint,
                            1000000
                        )
                    )
                ), 0)::bigint as agent_share,
                COUNT(
                    CASE WHEN bl.fee_amount > GREATEST(
                        (bl.gross_amount::numeric * $4::numeric)::bigint,
                        1000000
                    ) THEN 1 END
                )::bigint as tx_count
            FROM billing_logs bl
            JOIN merchants m ON bl.merchant_id = m.id
            WHERE m.referred_by_agent_id = $1
              AND bl.type = 'payment_credit'
              AND bl.fee_amount IS NOT NULL
              AND bl.fee_amount > 0
              AND bl.created_at >= $2::timestamptz
              AND bl.created_at < $3::timestamptz
            GROUP BY bl.merchant_id, m.name
            ORDER BY agent_share DESC
        "#;

        let base_rate_str = agent.base_rate.to_string();
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                sql,
                vec![
                    agent_id.into(),
                    start_date.into(),
                    end_date.into(),
                    base_rate_str.into(),
                ],
            ))
            .await?;

        let mut merchants_data = Vec::new();
        let mut total_fee: i64 = 0;
        let mut total_platform: i64 = 0;
        let mut total_agent: i64 = 0;
        let mut total_tx: i64 = 0;

        for row in &rows {
            let merchant_id: String = row.try_get("", "merchant_id")?;
            let merchant_name: String = row.try_get("", "merchant_name")?;
            let fee: i64 = row.try_get("", "total_fee")?;
            let platform: i64 = row.try_get("", "platform_share")?;
            let agent: i64 = row.try_get("", "agent_share")?;
            let count: i64 = row.try_get("", "tx_count")?;

            total_fee += fee;
            total_platform += platform;
            total_agent += agent;
            total_tx += count;

            merchants_data.push(ReferredMerchantCommission {
                merchant_id,
                merchant_name,
                total_fee_collected: fee,
                ironixpay_share: platform,
                agent_commission: agent,
                transaction_count: count,
            });
        }

        Ok(CommissionReport {
            agent_id: agent_id.to_string(),
            period_start: start_date.to_string(),
            period_end: end_date.to_string(),
            total_fee_collected: total_fee,
            total_ironixpay_share: total_platform,
            total_agent_commission: total_agent,
            total_transactions: total_tx,
            merchants: merchants_data,
        })
    }

    /// Dashboard: get agent overview for current merchant
    pub async fn get_overview(&self, merchant_id: &str) -> Result<AgentOverview> {
        let agent = self.find_by_merchant_id(merchant_id).await?;

        match agent {
            Some(a) => {
                let referred_count = self.count_referred_merchants(&a.id).await?;

                // Quick total commission (all time)
                let now = Utc::now();
                let start = "2020-01-01T00:00:00Z";
                let end = now.to_rfc3339();
                let report = self.get_commission_report(&a.id, start, &end).await?;

                Ok(AgentOverview {
                    is_agent: true,
                    agent_id: Some(a.id),
                    referral_code: Some(a.referral_code),
                    base_rate: Some(format!(
                        "{}%",
                        (a.base_rate * Decimal::from(100)).round_dp(2)
                    )),
                    max_markup: Some(format!(
                        "{}%",
                        (a.max_markup * Decimal::from(100)).round_dp(2)
                    )),
                    default_merchant_rate: Some(format!(
                        "{}%",
                        (a.default_merchant_rate * Decimal::from(100)).round_dp(2)
                    )),
                    referred_merchant_count: referred_count,
                    total_commission: report.total_agent_commission,
                })
            }
            None => Ok(AgentOverview {
                is_agent: false,
                agent_id: None,
                referral_code: None,
                base_rate: None,
                max_markup: None,
                default_merchant_rate: None,
                referred_merchant_count: 0,
                total_commission: 0,
            }),
        }
    }
}
