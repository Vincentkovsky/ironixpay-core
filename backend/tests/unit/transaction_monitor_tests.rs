use anyhow::Result;
use async_trait::async_trait;
use ironix_pay::entity::transactions::ChainTxState;
use ironix_pay::services::transaction_monitor::service::TransactionMonitor;
use ironix_pay::services::tron::interface::{
    AccountResource, BlockInfo, BroadcastResult, SignedTransaction, TransactionInfo,
    TronBroadcaster, UnsignedTransaction,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

struct MockTronClient {
    expiration: Option<i64>,
    current_block_time: i64,
}

#[async_trait]
impl TronBroadcaster for MockTronClient {
    async fn get_usdt_balance(&self, _address: &str) -> Result<u64> {
        Ok(0)
    }
    async fn get_trc20_balance(&self, _address: &str, _token_contract: &str) -> Result<i64> {
        Ok(0)
    }
    async fn get_trx_balance(&self, _address: &str) -> Result<u64> {
        Ok(0)
    }
    async fn build_trc20_transfer(
        &self,
        _f: &str,
        _t: &str,
        _a: u64,
        _c: &str,
    ) -> Result<UnsignedTransaction> {
        unimplemented!()
    }
    fn sign_transaction(&self, _tx: &UnsignedTransaction, _pk: &[u8]) -> Result<SignedTransaction> {
        unimplemented!()
    }
    async fn broadcast(&self, _tx: &SignedTransaction) -> Result<BroadcastResult> {
        unimplemented!()
    }
    async fn build_trx_transfer(&self, _f: &str, _t: &str, _a: u64) -> Result<UnsignedTransaction> {
        unimplemented!()
    }
    async fn get_account_resources(&self, _address: &str) -> Result<AccountResource> {
        Ok(AccountResource::default())
    }
    async fn estimate_energy(&self, _o: &str, _c: &str, _f: &str, _p: &str) -> Result<i64> {
        Ok(0)
    }

    async fn get_current_block(&self) -> Result<BlockInfo> {
        Ok(BlockInfo {
            number: 1000,
            timestamp: self.current_block_time,
        })
    }

    async fn get_transaction_info(&self, _tx_hash: &str) -> Result<Option<TransactionInfo>> {
        Ok(None) // No receipt yet
    }

    async fn get_transaction_by_id(&self, tx_hash: &str) -> Result<Option<SignedTransaction>> {
        Ok(Some(SignedTransaction {
            tx_id: tx_hash.to_string(),
            raw_data: vec![],
            signature: vec![],
            raw_data_json: None,
            expiration: self.expiration,
        }))
    }
}

#[tokio::test]
async fn test_monitor_transaction_expired() -> Result<()> {
    // Transaction expires at T=100 (long ago)
    let client = Arc::new(MockTronClient {
        expiration: Some(100),
        current_block_time: 101,
    });
    let monitor = TransactionMonitor::new(client);

    let status = monitor.check_tx_status("some_tx", 0, None).await?;
    assert_eq!(
        status,
        ChainTxState::NotFound,
        "Should be NotFound because it's expired"
    );
    Ok(())
}

#[tokio::test]
async fn test_monitor_transaction_pending() -> Result<()> {
    // Transaction expires far in the future
    let client = Arc::new(MockTronClient {
        expiration: Some(i64::MAX / 2),
        current_block_time: 101,
    });
    let monitor = TransactionMonitor::new(client);

    let status = monitor.check_tx_status("some_tx", 0, None).await?;
    assert_eq!(
        status,
        ChainTxState::Pending,
        "Should be Pending because it hasn't expired yet"
    );
    Ok(())
}

#[tokio::test]
async fn test_monitor_transaction_within_buffer() -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?;

    // Expires 10s ago, should still be PENDING due to 60s buffer
    let client = Arc::new(MockTronClient {
        expiration: Some(now - 10_000),
        current_block_time: now,
    });
    let monitor = TransactionMonitor::new(client);

    let status = monitor.check_tx_status("some_tx", 0, None).await?;
    assert_eq!(
        status,
        ChainTxState::Pending,
        "Should be Pending within 60s buffer"
    );
    Ok(())
}

#[tokio::test]
async fn test_monitor_transaction_outside_buffer() -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?;

    // Expires 70s ago, should be NOT_FOUND
    let client = Arc::new(MockTronClient {
        expiration: Some(now - 70_000),
        current_block_time: now,
    });
    let monitor = TransactionMonitor::new(client);

    let status = monitor.check_tx_status("some_tx", 0, None).await?;
    assert_eq!(
        status,
        ChainTxState::NotFound,
        "Should be NotFound outside 60s buffer"
    );
    Ok(())
}

#[tokio::test]
async fn test_monitor_transaction_unconfirmed_with_injected_height() -> Result<()> {
    // Transaction in block 1000
    // We inject block 1002
    // Req 5 confirmations -> should be Unconfirmed (only 2)
    struct CustomMock;
    #[async_trait]
    impl TronBroadcaster for CustomMock {
        async fn get_usdt_balance(&self, _a: &str) -> Result<u64> {
            Ok(0)
        }
        async fn get_trc20_balance(&self, _a: &str, _token_contract: &str) -> Result<i64> {
            Ok(0)
        }
        async fn get_trx_balance(&self, _a: &str) -> Result<u64> {
            Ok(0)
        }
        async fn build_trc20_transfer(
            &self,
            _f: &str,
            _t: &str,
            _a: u64,
            _c: &str,
        ) -> Result<UnsignedTransaction> {
            unimplemented!()
        }
        fn sign_transaction(
            &self,
            _tx: &UnsignedTransaction,
            _pk: &[u8],
        ) -> Result<SignedTransaction> {
            unimplemented!()
        }
        async fn broadcast(&self, _tx: &SignedTransaction) -> Result<BroadcastResult> {
            unimplemented!()
        }
        async fn build_trx_transfer(
            &self,
            _f: &str,
            _t: &str,
            _a: u64,
        ) -> Result<UnsignedTransaction> {
            unimplemented!()
        }
        async fn get_account_resources(&self, _address: &str) -> Result<AccountResource> {
            Ok(AccountResource::default())
        }
        async fn estimate_energy(&self, _o: &str, _c: &str, _f: &str, _p: &str) -> Result<i64> {
            Ok(0)
        }
        async fn get_current_block(&self) -> Result<BlockInfo> {
            panic!("Should not be called")
        }
        async fn get_transaction_by_id(&self, _h: &str) -> Result<Option<SignedTransaction>> {
            Ok(None)
        }

        async fn get_transaction_info(&self, h: &str) -> Result<Option<TransactionInfo>> {
            Ok(Some(TransactionInfo {
                tx_hash: h.to_string(),
                block_number: 1000,
                success: true,
                result: Some("SUCCESS".to_string()),
                fee_burned: 0,
                revert_message: None,
            }))
        }
    }

    let client = Arc::new(CustomMock);
    let monitor = TransactionMonitor::new(client);

    let status = monitor.check_tx_status("some_tx", 5, Some(1002)).await?;
    assert_eq!(status, ChainTxState::Unconfirmed);
    Ok(())
}
