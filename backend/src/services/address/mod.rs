//! Address Management Module with HD Wallet Support

pub mod hd_wallet;
pub mod key_provider;
pub mod service;

pub use service::{AddressAllocationError, AddressManager, InitializeAddressesResult};
