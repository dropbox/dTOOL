//! Common utilities for checkpointer implementations
//!
//! This module provides shared functionality used by external checkpointer backends
//! (Postgres, Redis, S3, DynamoDB) to reduce code duplication.
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow::checkpointer_helpers::{timestamp_to_nanos, nanos_to_timestamp};
//! use std::time::SystemTime;
//!
//! let nanos = timestamp_to_nanos(SystemTime::now());
//! let time = nanos_to_timestamp(nanos);
//! ```

use std::time::{Duration, SystemTime};

/// Convert `SystemTime` to Unix timestamp in nanoseconds
///
/// Returns 0 for times before the Unix epoch (instead of panicking).
/// For times far in the future, the value is clamped to `i64::MAX`.
///
/// # Examples
///
/// ```
/// use dashflow::checkpointer_helpers::timestamp_to_nanos;
/// use std::time::SystemTime;
///
/// let now = SystemTime::now();
/// let nanos = timestamp_to_nanos(now);
/// assert!(nanos > 0);
/// ```
#[must_use]
pub fn timestamp_to_nanos(time: SystemTime) -> i64 {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => {
            // u128 can be larger than i64::MAX, so clamp
            let nanos = duration.as_nanos();
            if nanos > i64::MAX as u128 {
                i64::MAX
            } else {
                nanos as i64
            }
        }
        Err(_) => {
            // Time is before Unix epoch - return 0 as a safe default
            0
        }
    }
}

/// Convert Unix timestamp in nanoseconds to `SystemTime`
///
/// Handles negative values (before epoch) by returning `UNIX_EPOCH`.
///
/// # Examples
///
/// ```
/// use dashflow::checkpointer_helpers::nanos_to_timestamp;
/// use std::time::SystemTime;
///
/// let time = nanos_to_timestamp(1_000_000_000_000_000_000); // ~31.7 years from epoch
/// assert!(time > SystemTime::UNIX_EPOCH);
///
/// // Negative values return UNIX_EPOCH
/// let epoch = nanos_to_timestamp(-100);
/// assert_eq!(epoch, SystemTime::UNIX_EPOCH);
/// ```
#[must_use]
pub fn nanos_to_timestamp(nanos: i64) -> SystemTime {
    if nanos < 0 {
        SystemTime::UNIX_EPOCH
    } else {
        SystemTime::UNIX_EPOCH + Duration::from_nanos(nanos as u64)
    }
}

/// Convert Unix timestamp in milliseconds to `SystemTime`
///
/// Useful for backends that store timestamps in milliseconds (like Redis ZSET scores).
///
/// # Examples
///
/// ```
/// use dashflow::checkpointer_helpers::millis_to_timestamp;
/// use std::time::SystemTime;
///
/// let time = millis_to_timestamp(1_000_000_000_000); // ~31.7 years from epoch
/// assert!(time > SystemTime::UNIX_EPOCH);
/// ```
#[must_use]
pub fn millis_to_timestamp(millis: i64) -> SystemTime {
    if millis < 0 {
        SystemTime::UNIX_EPOCH
    } else {
        SystemTime::UNIX_EPOCH + Duration::from_millis(millis as u64)
    }
}

/// Convert `SystemTime` to Unix timestamp in milliseconds
///
/// Useful for backends that need millisecond precision (like Redis ZSET scores).
///
/// # Examples
///
/// ```
/// use dashflow::checkpointer_helpers::timestamp_to_millis;
/// use std::time::SystemTime;
///
/// let now = SystemTime::now();
/// let millis = timestamp_to_millis(now);
/// assert!(millis > 0);
/// ```
#[must_use]
pub fn timestamp_to_millis(time: SystemTime) -> i64 {
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => {
            let millis = duration.as_millis();
            if millis > i64::MAX as u128 {
                i64::MAX
            } else {
                millis as i64
            }
        }
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_to_nanos_positive() {
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let nanos = timestamp_to_nanos(time);
        assert_eq!(nanos, 1_000_000_000_000); // 1000 seconds in nanoseconds
    }

    #[test]
    fn test_timestamp_to_nanos_now() {
        let now = SystemTime::now();
        let nanos = timestamp_to_nanos(now);
        assert!(nanos > 0);
    }

    #[test]
    fn test_nanos_to_timestamp_positive() {
        let nanos = 1_000_000_000_000_i64; // 1000 seconds in nanoseconds
        let time = nanos_to_timestamp(nanos);
        assert_eq!(time, SystemTime::UNIX_EPOCH + Duration::from_secs(1000));
    }

    #[test]
    fn test_nanos_to_timestamp_negative() {
        let nanos = -100_i64;
        let time = nanos_to_timestamp(nanos);
        assert_eq!(time, SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn test_roundtrip_nanos() {
        let original = SystemTime::now();
        let nanos = timestamp_to_nanos(original);
        let roundtrip = nanos_to_timestamp(nanos);

        // Should be within 1 nanosecond (may lose sub-nanosecond precision)
        let diff = if original > roundtrip {
            original.duration_since(roundtrip).unwrap()
        } else {
            roundtrip.duration_since(original).unwrap()
        };
        assert!(diff < Duration::from_nanos(1000)); // Allow for tiny precision loss
    }

    #[test]
    fn test_timestamp_to_millis() {
        let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let millis = timestamp_to_millis(time);
        assert_eq!(millis, 1_000_000); // 1000 seconds in milliseconds
    }

    #[test]
    fn test_millis_to_timestamp() {
        let millis = 1_000_000_i64; // 1000 seconds in milliseconds
        let time = millis_to_timestamp(millis);
        assert_eq!(time, SystemTime::UNIX_EPOCH + Duration::from_secs(1000));
    }

    #[test]
    fn test_roundtrip_millis() {
        let original = SystemTime::UNIX_EPOCH + Duration::from_millis(1_234_567);
        let millis = timestamp_to_millis(original);
        let roundtrip = millis_to_timestamp(millis);
        assert_eq!(original, roundtrip);
    }
}
