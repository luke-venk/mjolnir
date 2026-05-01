use std::sync::atomic::{AtomicI64, Ordering};
#[cfg(not(test))]
use std::sync::OnceLock;
#[cfg(test)]
use std::sync::RwLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[cfg(not(test))]
static GLOBAL_TIME: OnceLock<GlobalTime> = OnceLock::new();

#[cfg(test)]
static GLOBAL_TIME: RwLock<Option<GlobalTime>> = RwLock::new(None);

/// A struct for tracking the global time of the program, including
/// - A monotonic Instant program start time
/// - A wall clock program start time time in nanoseconds since the Unix epoch
/// - An approximate value to add to that wall clock to obtain an estimate of the current camera PTP time
///
/// This allows us to have a consistent reference point for time measurements,
/// regardless of any SystemTime adjustments.
#[cfg(not(test))]
pub fn global_time() -> &'static GlobalTime {
    GLOBAL_TIME
        .get()
        .expect("GlobalTime not initialized — call init_global_time() first")
}

#[cfg(test)]
pub fn global_time() -> GlobalTime {
    GLOBAL_TIME
        .read()
        .expect("GlobalTime lock poisoned")
        .as_ref()
        .expect("GlobalTime not initialized — call init_global_time() first")
        .clone()
}

/// Initializes the global time. This should be called once at the start of the program.
#[cfg(not(test))]
pub fn init_global_time() {
    GLOBAL_TIME
        .set(GlobalTime::new())
        .expect("GlobalTime already initialized — init_global_time() called twice");
}

#[cfg(test)]
pub fn init_global_time() {
    *GLOBAL_TIME.write().expect("GlobalTime lock poisoned") = Some(GlobalTime::new());
}

/// A struct for tracking the global start times of the program accurately
#[derive(Debug)]
pub struct GlobalTime {
    program_start_time_instant: Instant,
    program_start_time_wall_clock_nanoseconds: u64,
    approximate_additive_ptp_offset_from_wall_clock_nanoseconds: AtomicI64,
}

impl GlobalTime {
    fn new() -> Self {
        let before = Instant::now();
        let wall = nanoseconds_since_unix_epoch_utc();
        let after = Instant::now();
        // Use the midpoint of before/after as the best estimate for when wall was sampled
        let program_start_time_instant = before + (after - before) / 2;
        Self {
            program_start_time_instant,
            program_start_time_wall_clock_nanoseconds: wall,
            approximate_additive_ptp_offset_from_wall_clock_nanoseconds: i64::MIN.into(),
        }
    }

    /// Returns the monotonic Instant when the program started.
    /// This is actually the average Instant of before and after the wall clock time was sampled.
    pub fn program_start_time_instant(&self) -> Instant {
        self.program_start_time_instant
    }

    /// Returns the wall clock time in nanoseconds since the Unix epoch when the program started.
    /// This is sampled from SystemTime.
    /// This is not affected by any adjustments to the system clock after the program starts.
    pub fn program_start_time_wall_clock_nanoseconds(&self) -> u64 {
        self.program_start_time_wall_clock_nanoseconds
    }

    /// Returns the current monotonic time in nanoseconds since the Unix epoch, based on the program start time.
    /// This is not affected by any adjustments to the system clock after the program starts.
    pub fn now_monotonic_in_nanoseconds_since_unix_epoch(&self) -> u64 {
        self.program_start_time_wall_clock_nanoseconds
            .saturating_add(self.program_start_time_instant.elapsed().as_nanos() as u64)
    }

    pub fn set_approximate_additive_ptp_offset_from_wall_clock_nanoseconds(
        &self,
        offset: Option<i64>,
    ) {
        let val = offset.unwrap_or(i64::MIN);
        self.approximate_additive_ptp_offset_from_wall_clock_nanoseconds
            .store(val, Ordering::Relaxed);
    }

    /// Returns an approximation of the current camera PTP time
    /// NOT guaranteed to be monotonic since callers could change the internal PTP offset
    /// NOT guaranteed to be highly accurate since we don't actually participate in a PTP sync with the cameras
    /// WILL drift over time since we do not participate in active PTP sync with the cameras
    pub fn camera_ptp_time_now_approximation_nanoseconds(&self) -> Option<u64> {
        match self
            .approximate_additive_ptp_offset_from_wall_clock_nanoseconds
            .load(Ordering::Relaxed)
        {
            i64::MIN => None,
            offset => Some(
                self.now_monotonic_in_nanoseconds_since_unix_epoch()
                    .saturating_add_signed(offset),
            ),
        }
    }
}

impl Clone for GlobalTime {
    fn clone(&self) -> Self {
        Self {
            program_start_time_instant: self.program_start_time_instant,
            program_start_time_wall_clock_nanoseconds: self
                .program_start_time_wall_clock_nanoseconds,
            approximate_additive_ptp_offset_from_wall_clock_nanoseconds: AtomicI64::new(
                self.approximate_additive_ptp_offset_from_wall_clock_nanoseconds
                    .load(Ordering::Relaxed),
            ),
        }
    }
}

/// Returns the current wall clock time in nanoseconds since the Unix epoch.
/// This is sampled directly from SystemTime, so it can be affected by adjustments to the system clock.
/// For a monotonic time reference, use GlobalTime instead.
pub fn nanoseconds_since_unix_epoch_utc() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System clock before 1970")
        .as_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::thread::sleep;
    use std::time::Duration;

    #[rstest]
    fn test_global_time_inits_with_program_start_time_now() {
        let now = Instant::now();

        let gt = GlobalTime::new();

        assert!(gt.program_start_time_instant() >= now);
        assert!(gt.program_start_time_instant() <= Instant::now());
    }

    #[rstest]
    fn test_nanoseconds_since_unix_epoch_utc_returns_value() {
        assert!(nanoseconds_since_unix_epoch_utc() > 0);
    }

    #[rstest]
    fn test_nanoseconds_since_unix_epoch_utc_advances_over_time() {
        let past = nanoseconds_since_unix_epoch_utc();
        sleep(Duration::from_millis(1));

        let present = nanoseconds_since_unix_epoch_utc();

        assert!(present - past > 0);
    }

    #[rstest]
    fn test_now_monotonic_in_nanoseconds_since_unix_epoch_starts_at_or_above_program_start_wall_clock(
    ) {
        let gt = GlobalTime::new();

        let monotonic_time = gt.now_monotonic_in_nanoseconds_since_unix_epoch();

        assert!(monotonic_time >= gt.program_start_time_wall_clock_nanoseconds());
    }

    #[rstest]
    fn test_now_monotonic_in_nanoseconds_since_unix_epoch_advances_over_time() {
        let gt = GlobalTime::new();
        let first = gt.now_monotonic_in_nanoseconds_since_unix_epoch();
        sleep(Duration::from_millis(10));

        let second = gt.now_monotonic_in_nanoseconds_since_unix_epoch();

        let delta = second - first;
        // Note that the range of acceptable values here is ridiculously wide
        // Because that is how imprecise the OS is with scheduling threads
        // When sleep is for 10ms, the OS might wait decently longer than that
        // to start runnnig the thread again.
        assert!(
            delta >= 5_000_000,
            "Monotonic time should have advanced by at least 8ms, but only advanced by {delta}µs"
        );
        assert!(
            delta <= 20_000_000,
            "Monotonic time should have advanced by at most 20ms, but advanced by {delta}µs"
        );
    }
}
