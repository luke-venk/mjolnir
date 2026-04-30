use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

/// Stores an estimate of the mapping between local time and camera PTP time.
///
/// We define:
///   offset_ns = ptp_ns - local_ns
///
/// Then an approximate PTP timestamp for an event observed at local_ns is:
///   ptp_est_ns = local_ns + offset_ns
///
/// This is intentionally simple: one shared offset for the process.
/// If you want smoothing later, call `update_offset_ns()` with an averaged value.
static HAS_OFFSET: AtomicBool = AtomicBool::new(false);
static OFFSET_NS: AtomicI64 = AtomicI64::new(0);

/// Update the global offset using a known pair of timestamps.
/// Both inputs should be in nanoseconds.
pub fn update_offset_from_pair(ptp_ns: u64, local_ns: u64) {
    // offset can be negative depending on what "local time" means; keep signed.
    let offset = ptp_ns as i128 - local_ns as i128;

    // Clamp to i64 range (extremely unlikely to overflow in practice).
    let offset_i64 = if offset > i64::MAX as i128 {
        i64::MAX
    } else if offset < i64::MIN as i128 {
        i64::MIN
    } else {
        offset as i64
    };

    OFFSET_NS.store(offset_i64, Ordering::Relaxed);
    HAS_OFFSET.store(true, Ordering::Release);
}

/// Returns the current offset (ptp_ns - local_ns) if initialized.
pub fn get_offset_ns() -> Option<i64> {
    if HAS_OFFSET.load(Ordering::Acquire) {
        Some(OFFSET_NS.load(Ordering::Relaxed))
    } else {
        None
    }
}

/// Convert a local timestamp (ns) into an approximate PTP timestamp (ns).
pub fn estimate_ptp_ns(local_ns: u64) -> Option<u64> {
    let offset = get_offset_ns()?;

    // local_ns + offset could underflow/overflow; guard it.
    let local_i128 = local_ns as i128;
    let ptp_i128 = local_i128 + offset as i128;

    if ptp_i128 < 0 || ptp_i128 > u64::MAX as i128 {
        None
    } else {
        Some(ptp_i128 as u64)
    }
}

/// Clear the stored offset (useful for tests).
pub fn clear() {
    HAS_OFFSET.store(false, Ordering::Release);
    OFFSET_NS.store(0, Ordering::Relaxed);
}