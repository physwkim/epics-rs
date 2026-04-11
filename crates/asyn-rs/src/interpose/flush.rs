//! Flush interpose layer.
//!
//! Corresponds to C asyn's `asynInterposeFlush.c`. On explicit flush, discards
//! any stale data by reading with a short timeout until nothing remains.
//! Read and write operations pass through unchanged.

use std::time::Duration;

use crate::error::AsynResult;
use crate::user::AsynUser;

use super::{OctetInterpose, OctetNext, OctetReadResult};

/// Flush interpose layer.
///
/// On `flush()`, temporarily sets a short timeout and reads in a loop to
/// discard stale data, matching C asyn's `asynInterposeFlush` semantics.
/// Read and write are pure pass-through.
pub struct FlushTimeoutInterpose {
    /// Timeout used during flush discard reads.
    pub flush_timeout: Duration,
}

impl FlushTimeoutInterpose {
    pub fn new(flush_timeout: Duration) -> Self {
        Self { flush_timeout }
    }
}

impl Default for FlushTimeoutInterpose {
    fn default() -> Self {
        // C default: 1 ms (minimum when timeout <= 0)
        Self::new(Duration::from_millis(1))
    }
}

impl OctetInterpose for FlushTimeoutInterpose {
    fn read(
        &mut self,
        user: &AsynUser,
        buf: &mut [u8],
        next: &mut dyn OctetNext,
    ) -> AsynResult<OctetReadResult> {
        // Pure pass-through (C parity: readIt just delegates to lower layer)
        next.read(user, buf)
    }

    fn write(
        &mut self,
        user: &mut AsynUser,
        data: &[u8],
        next: &mut dyn OctetNext,
    ) -> AsynResult<usize> {
        // Pure pass-through
        next.write(user, data)
    }

    fn flush(&mut self, user: &mut AsynUser, next: &mut dyn OctetNext) -> AsynResult<()> {
        // Save the user's original timeout and set our short flush timeout
        let save_timeout = user.timeout;
        user.timeout = self.flush_timeout;

        // Discard stale data by reading until nothing comes back
        let mut buffer = [0u8; 100];
        loop {
            match next.read(user, &mut buffer) {
                Ok(result) if result.nbytes_transferred > 0 => continue,
                _ => break,
            }
        }

        // Restore original timeout
        user.timeout = save_timeout;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use crate::interpose::EomReason;

    struct FlushableBase {
        read_count: Arc<AtomicUsize>,
        reads_with_data: usize,
    }

    impl FlushableBase {
        fn new(reads_with_data: usize) -> Self {
            Self {
                read_count: Arc::new(AtomicUsize::new(0)),
                reads_with_data,
            }
        }
    }

    impl OctetNext for FlushableBase {
        fn read(&mut self, _user: &AsynUser, buf: &mut [u8]) -> AsynResult<OctetReadResult> {
            let n = self.read_count.fetch_add(1, Ordering::Relaxed);
            if n < self.reads_with_data {
                let msg = b"stale";
                let len = msg.len().min(buf.len());
                buf[..len].copy_from_slice(&msg[..len]);
                Ok(OctetReadResult {
                    nbytes_transferred: len,
                    eom_reason: EomReason::CNT,
                })
            } else {
                Ok(OctetReadResult {
                    nbytes_transferred: 0,
                    eom_reason: EomReason::CNT,
                })
            }
        }

        fn write(&mut self, _user: &mut AsynUser, data: &[u8]) -> AsynResult<usize> {
            Ok(data.len())
        }

        fn flush(&mut self, _user: &mut AsynUser) -> AsynResult<()> {
            Ok(())
        }
    }

    #[test]
    fn test_flush_discards_stale_data() {
        let mut interpose = FlushTimeoutInterpose::new(Duration::from_millis(10));
        let mut base = FlushableBase::new(2); // 2 reads return stale data
        let mut user = AsynUser::default();

        interpose.flush(&mut user, &mut base).unwrap();
        // Should have done 2 stale reads + 1 empty read (breaks loop) = 3
        assert!(base.read_count.load(Ordering::Relaxed) >= 3);
    }

    #[test]
    fn test_read_passthrough() {
        let mut interpose = FlushTimeoutInterpose::default();
        let mut base = FlushableBase::new(1); // 1 read returns data
        let user = AsynUser::default();
        let mut buf = [0u8; 32];

        let result = interpose.read(&user, &mut buf, &mut base).unwrap();
        assert_eq!(&buf[..result.nbytes_transferred], b"stale");
    }

    #[test]
    fn test_write_passthrough() {
        let mut interpose = FlushTimeoutInterpose::default();
        let mut base = FlushableBase::new(0);
        let mut user = AsynUser::default();

        let n = interpose.write(&mut user, b"hello", &mut base).unwrap();
        assert_eq!(n, 5);
    }

    #[test]
    fn test_flush_restores_timeout() {
        let mut interpose = FlushTimeoutInterpose::new(Duration::from_millis(10));
        let mut base = FlushableBase::new(0);
        let original_timeout = Duration::from_secs(5);
        let mut user = AsynUser::default();
        user.timeout = original_timeout;

        interpose.flush(&mut user, &mut base).unwrap();
        assert_eq!(user.timeout, original_timeout);
    }
}
