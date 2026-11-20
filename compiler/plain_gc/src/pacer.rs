//! The pacer: when a collection starts (D113), aiming at the heap's soft target and kept under its hard limit (D114).
//!
//! As ZGC's does, it measures how fast the program allocates and how long a collection takes, predicts when the heap
//! would reach its soft target at that speed, and starts a collection just early enough to finish before then. Each
//! measurement is a decaying average with its spread, and the prediction takes the average plus one spread, so a
//! program that allocates in bursts is paced by its bursts. While it has no measurements yet, it warms up with early
//! collections; and when the program has gone a long while without one, it collects so memory it no longer uses
//! isn't held.

use std::time::{Duration, Instant};

/// The soft target's least size. The pacer's own, which no program can see (D114).
pub const LEAST_SOFT_TARGET: usize = 32 * 1024 * 1024;

/// The soft target is this many times what was live after the last collection, like ZGC's `SoftMaxHeapSize` (D114).
const SOFT_TARGET_TIMES_LIVE: usize = 2;

/// The hard limit leaves the system at least this much of its memory (D114)...
const LEAST_RESERVE: usize = 1024 * 1024 * 1024;

/// ...or this share of it, in percent, whichever is bigger (D114).
const RESERVE_PERCENT: usize = 10;

/// How many collections warm-up has: the first at 10% of the soft target, the next at 20%, then 30%, as ZGC's.
const WARM_UP_COLLECTIONS: usize = 3;

/// The least time between two measurements of the allocation rate.
const SAMPLE_INTERVAL: Duration = Duration::from_millis(10);

/// After this long without a collection, the next allocation starts one, so memory isn't held for long (D113).
const QUIET_TIME: Duration = Duration::from_secs(5 * 60);

/// How much a new measurement counts, against the average so far.
const NEWEST_WEIGHT: f64 = 0.3;

/// A decaying average and its spread.
#[derive(Clone, Copy, Debug, Default)]
struct Average {
    mean: f64,
    variance: f64,
    samples: usize,
}

impl Average {
    fn add(&mut self, value: f64) {
        if self.samples == 0 {
            self.mean = value;
            self.variance = 0.0;
        } else {
            let difference = value - self.mean;
            self.mean += NEWEST_WEIGHT * difference;
            self.variance =
                (1.0 - NEWEST_WEIGHT) * (self.variance + NEWEST_WEIGHT * difference * difference);
        }
        self.samples += 1;
    }

    /// The average plus one spread: a cautious prediction of the next measurement.
    fn predict(&self) -> f64 {
        self.mean + self.variance.sqrt()
    }
}

/// Why a collection starts, for tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    WarmUp,
    AllocationRate,
    Quiet,
}

pub struct Pacer {
    /// Bytes allocated per second.
    allocation_rate: Average,
    /// Seconds a collection takes, from its start to the end of its relocation.
    duration: Average,
    last_sample: Instant,
    allocated_at_last_sample: u64,
    /// When the collection under way started.
    started: Option<Instant>,
    last_collection_ended: Instant,
    completed: usize,
}

impl Pacer {
    pub fn new(now: Instant) -> Pacer {
        Pacer {
            allocation_rate: Average::default(),
            duration: Average::default(),
            last_sample: now,
            allocated_at_last_sample: 0,
            started: None,
            last_collection_ended: now,
            completed: 0,
        }
    }

    /// Measures the allocation rate, at most once per [`SAMPLE_INTERVAL`]. `allocated` counts every byte ever
    /// allocated.
    pub fn sample(&mut self, now: Instant, allocated: u64) {
        let elapsed = now.duration_since(self.last_sample);
        if elapsed < SAMPLE_INTERVAL {
            return;
        }
        let bytes = allocated.saturating_sub(self.allocated_at_last_sample);
        self.allocation_rate
            .add(bytes as f64 / elapsed.as_secs_f64());
        self.last_sample = now;
        self.allocated_at_last_sample = allocated;
    }

    /// Whether to start a collection now, with `used` bytes in the heap, `allocated_since` of them allocated since the
    /// last collection, and the heap's soft target.
    pub fn should_start(
        &self,
        now: Instant,
        used: usize,
        allocated_since: usize,
        soft_target: usize,
    ) -> Option<Reason> {
        // trace: D113
        if self.completed < WARM_UP_COLLECTIONS {
            let threshold = soft_target / 10 * (self.completed + 1);
            return (used >= threshold).then_some(Reason::WarmUp);
        }
        let free = soft_target.saturating_sub(used) as f64;
        let rate = self.allocation_rate.predict().max(1.0);
        let seconds_until_target = free / rate;
        let seconds_needed = self.duration.predict() + SAMPLE_INTERVAL.as_secs_f64();
        if seconds_until_target <= seconds_needed {
            return Some(Reason::AllocationRate);
        }
        if allocated_since > 0 && now.duration_since(self.last_collection_ended) >= QUIET_TIME {
            return Some(Reason::Quiet);
        }
        None
    }

    pub fn collection_started(&mut self, now: Instant) {
        self.started = Some(now);
    }

    pub fn collection_ended(&mut self, now: Instant) {
        if let Some(started) = self.started.take() {
            self.duration.add(now.duration_since(started).as_secs_f64());
        }
        self.last_collection_ended = now;
        self.completed += 1;
    }
}

/// The soft target, where the pacer aims (D114): twice what was live after the last collection, at least
/// [`LEAST_SOFT_TARGET`], and never past the hard limit.
pub fn soft_target(live_after_collection: usize, hard_limit: usize) -> usize {
    // trace: D114
    live_after_collection
        .saturating_mul(SOFT_TARGET_TIMES_LIVE)
        .max(LEAST_SOFT_TARGET)
        .min(hard_limit)
}

/// The hard limit (D114): the machine's physical memory, less a reserve of 10% of it or 1 GiB, whichever is bigger.
/// With no way to read the machine's memory, there's no limit but the system's own.
pub fn hard_limit() -> usize {
    // trace: D114
    match physical_memory() {
        Some(memory) => {
            memory.saturating_sub((memory.div_ceil(100) * RESERVE_PERCENT).max(LEAST_RESERVE))
        }
        None => usize::MAX,
    }
}

#[cfg(target_os = "macos")]
fn physical_memory() -> Option<usize> {
    unsafe extern "C" {
        fn sysctlbyname(
            name: *const std::ffi::c_char,
            old: *mut std::ffi::c_void,
            old_length: *mut usize,
            new: *mut std::ffi::c_void,
            new_length: usize,
        ) -> i32;
    }
    let mut memory: u64 = 0;
    let mut length = std::mem::size_of::<u64>();
    // SAFETY: `hw.memsize` is a 64-bit number, and `memory` and `length` describe 8 writable bytes for it.
    let result = unsafe {
        sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&raw mut memory).cast(),
            &raw mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    (result == 0 && memory > 0).then(|| usize::try_from(memory).unwrap_or(usize::MAX))
}

#[cfg(target_os = "linux")]
fn physical_memory() -> Option<usize> {
    unsafe extern "C" {
        fn sysconf(name: i32) -> i64;
    }
    const PHYSICAL_PAGES: i32 = 85;
    const PAGE_SIZE: i32 = 30;
    // SAFETY: `sysconf` only reads the system's configuration.
    let (pages, page_size) = unsafe { (sysconf(PHYSICAL_PAGES), sysconf(PAGE_SIZE)) };
    let memory = u64::try_from(pages)
        .ok()?
        .checked_mul(u64::try_from(page_size).ok()?)?;
    (memory > 0).then(|| usize::try_from(memory).unwrap_or(usize::MAX))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn physical_memory() -> Option<usize> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: usize = 1024 * 1024;

    #[test]
    fn d114_the_hard_limit_leaves_a_reserve() {
        let limit = hard_limit();
        if let Some(memory) = physical_memory() {
            assert!(limit <= memory - LEAST_RESERVE.min(memory));
            assert!(limit <= memory - memory / 10);
        }
    }

    #[test]
    fn d114_the_soft_target_is_twice_whats_live() {
        assert_eq!(soft_target(100 * MIB, usize::MAX), 200 * MIB);
        assert_eq!(soft_target(MIB, usize::MAX), LEAST_SOFT_TARGET);
        assert_eq!(soft_target(100 * MIB, 150 * MIB), 150 * MIB);
    }

    #[test]
    fn d113_warm_up_collects_early_three_times() {
        let start = Instant::now();
        let mut pacer = Pacer::new(start);
        let target = 100 * MIB;
        assert_eq!(pacer.should_start(start, 9 * MIB, 9 * MIB, target), None);
        assert_eq!(
            pacer.should_start(start, 10 * MIB, 10 * MIB, target),
            Some(Reason::WarmUp)
        );
        pacer.collection_started(start);
        pacer.collection_ended(start);
        assert_eq!(pacer.should_start(start, 15 * MIB, 5 * MIB, target), None);
        assert_eq!(
            pacer.should_start(start, 20 * MIB, 10 * MIB, target),
            Some(Reason::WarmUp)
        );
    }

    #[test]
    fn d113_a_collection_starts_just_early_enough_to_finish() {
        let start = Instant::now();
        let mut pacer = Pacer::new(start);
        // Three collections of 100 milliseconds each, while allocating 100 MiB a second.
        let mut now = start;
        let mut allocated = 0;
        for _ in 0..WARM_UP_COLLECTIONS {
            pacer.collection_started(now);
            for _ in 0..10 {
                now += Duration::from_millis(10);
                allocated += MIB as u64;
                pacer.sample(now, allocated);
            }
            pacer.collection_ended(now);
        }
        let target = 100 * MIB;
        // 50 MiB to go is half a second away, and a collection takes a tenth: not yet.
        assert_eq!(pacer.should_start(now, 50 * MIB, 50 * MIB, target), None);
        // 10 MiB to go is a tenth of a second away: time to start.
        assert_eq!(
            pacer.should_start(now, 90 * MIB, 90 * MIB, target),
            Some(Reason::AllocationRate)
        );
    }

    #[test]
    fn d113_a_quiet_program_still_collects_now_and_then() {
        let start = Instant::now();
        let mut pacer = Pacer::new(start);
        for _ in 0..WARM_UP_COLLECTIONS {
            pacer.collection_started(start);
            pacer.collection_ended(start);
        }
        let later = start + QUIET_TIME;
        assert_eq!(pacer.should_start(later, MIB, 0, 100 * MIB), None);
        assert_eq!(
            pacer.should_start(later, MIB, 1024, 100 * MIB),
            Some(Reason::Quiet)
        );
    }
}
