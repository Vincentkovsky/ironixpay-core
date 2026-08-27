//! Transaction Indexer Module

pub mod scanner;
pub mod service;
pub mod sync;

pub use scanner::BlockScanner;
pub use service::{IndexerStats, TransactionIndexer};
pub use sync::AddressSyncManager;

/// Information about a monitored address (from addresses table)
#[derive(Clone, Debug)]
pub struct MonitoredAddressInfo {
    pub merchant_id: String,
}
