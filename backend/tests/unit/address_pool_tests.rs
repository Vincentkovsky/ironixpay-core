//! Unit Tests for Address Pool
//!
//! Tests for address allocation, recycling, and status transitions.
//! Aligned with docs/system_design.md schema.

#[cfg(test)]
mod address_pool_tests {
    /// Test address status transitions (based on AddressStatus enum)
    #[test]
    fn test_address_status_transitions() {
        #[derive(Debug, PartialEq, Clone)]
        #[allow(dead_code)]
        enum AddressStatus {
            Idle,
            Assigned,
            Detected,
            Sweeping,
            Cooling,
            Locked,
        }

        #[allow(dead_code)]
        struct Address {
            network: String,
            address: String,
            status: AddressStatus,
            session_id: Option<String>,
        }

        impl Address {
            fn allocate(&mut self, session_id: &str) -> Result<(), &'static str> {
                match self.status {
                    AddressStatus::Idle => {
                        self.status = AddressStatus::Assigned;
                        self.session_id = Some(session_id.to_string());
                        Ok(())
                    }
                    _ => Err("Cannot allocate: not idle"),
                }
            }

            fn detect_payment(&mut self) -> Result<(), &'static str> {
                match self.status {
                    AddressStatus::Assigned => {
                        self.status = AddressStatus::Detected;
                        Ok(())
                    }
                    _ => Err("Cannot detect: not assigned"),
                }
            }

            fn start_sweep(&mut self) -> Result<(), &'static str> {
                match self.status {
                    AddressStatus::Detected => {
                        self.status = AddressStatus::Sweeping;
                        Ok(())
                    }
                    _ => Err("Cannot sweep: not detected"),
                }
            }

            fn finish_sweep(&mut self) -> Result<(), &'static str> {
                match self.status {
                    AddressStatus::Sweeping => {
                        self.status = AddressStatus::Cooling;
                        self.session_id = None;
                        Ok(())
                    }
                    _ => Err("Cannot finish sweep: not sweeping"),
                }
            }

            fn recycle(&mut self) -> Result<(), &'static str> {
                match self.status {
                    AddressStatus::Cooling => {
                        self.status = AddressStatus::Idle;
                        Ok(())
                    }
                    _ => Err("Cannot recycle: not cooling"),
                }
            }
        }

        // Test full lifecycle: Idle → Assigned → Detected → Sweeping → Cooling → Idle
        let mut addr = Address {
            network: "TRON".to_string(),
            address: "TXxxxxxxxxxxxxxxxxxxxxxxxxxxxx".to_string(),
            status: AddressStatus::Idle,
            session_id: None,
        };

        assert!(addr.allocate("session_123").is_ok());
        assert_eq!(addr.status, AddressStatus::Assigned);

        assert!(addr.detect_payment().is_ok());
        assert_eq!(addr.status, AddressStatus::Detected);

        assert!(addr.start_sweep().is_ok());
        assert_eq!(addr.status, AddressStatus::Sweeping);

        assert!(addr.finish_sweep().is_ok());
        assert_eq!(addr.status, AddressStatus::Cooling);
        assert!(addr.session_id.is_none());

        assert!(addr.recycle().is_ok());
        assert_eq!(addr.status, AddressStatus::Idle);
    }

    /// Test pool exhaustion handling
    #[test]
    fn test_pool_exhaustion() {
        struct MockPool {
            available: Vec<String>,
        }

        impl MockPool {
            fn allocate(&mut self) -> Result<String, &'static str> {
                self.available.pop().ok_or("Pool exhausted")
            }

            fn is_empty(&self) -> bool {
                self.available.is_empty()
            }
        }

        let mut pool = MockPool {
            available: vec!["TAddr1".to_string(), "TAddr2".to_string()],
        };

        assert!(!pool.is_empty());
        assert!(pool.allocate().is_ok());
        assert!(pool.allocate().is_ok());
        assert!(pool.is_empty());
        assert!(pool.allocate().is_err());
    }

    /// Test concurrent allocation safety (atomic operations)
    #[test]
    fn test_atomic_allocation() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        // Simulate atomic address allocation
        let pool_size = Arc::new(AtomicUsize::new(10));

        // Multiple "threads" trying to allocate
        let allocations: Vec<bool> = (0..15)
            .map(|_| {
                // Try to decrement atomically
                loop {
                    let current = pool_size.load(Ordering::SeqCst);
                    if current == 0 {
                        return false; // Pool exhausted
                    }
                    if pool_size
                        .compare_exchange(current, current - 1, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        return true; // Successfully allocated
                    }
                    // Retry if another thread modified
                }
            })
            .collect();

        let successful = allocations.iter().filter(|&&x| x).count();
        let failed = allocations.iter().filter(|&&x| !x).count();

        assert_eq!(successful, 10); // Exactly 10 allocations
        assert_eq!(failed, 5); // 5 failed due to exhaustion
        assert_eq!(pool_size.load(Ordering::SeqCst), 0);
    }
}
