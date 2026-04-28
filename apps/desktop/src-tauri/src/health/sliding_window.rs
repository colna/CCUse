//! T1.0.2.05 — [`SlidingWindow`] for success-rate calculation.
//!
//! Fixed-capacity ring buffer of booleans. The `HealthChecker` pushes
//! one sample per probe; strategies read `success_rate()` to decide
//! whether a provider is `Healthy`, `Degraded`, or `Down`.

use std::collections::VecDeque;

/// Bounded ring buffer that tracks recent probe outcomes.
#[derive(Debug, Clone)]
pub struct SlidingWindow {
    buffer: VecDeque<bool>,
    capacity: usize,
}

impl SlidingWindow {
    /// Create a window that holds at most `capacity` samples.
    /// Panics if `capacity` is 0.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "SlidingWindow capacity must be > 0");
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a probe outcome. If the buffer is full, the oldest sample
    /// is evicted.
    pub fn push(&mut self, success: bool) {
        if self.buffer.len() == self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(success);
    }

    /// Fraction of successful probes in `[0.0, 1.0]`. Returns `1.0`
    /// (optimistic) when no samples have been recorded yet.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // window capacity ≤ 100
    pub fn success_rate(&self) -> f64 {
        if self.buffer.is_empty() {
            return 1.0;
        }
        let ok = self.buffer.iter().filter(|&&s| s).count();
        ok as f64 / self.buffer.len() as f64
    }

    /// Number of samples currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether any samples have been recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Maximum number of samples.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all recorded samples.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact ratios from small integers
mod tests {
    use super::*;

    #[test]
    fn empty_window_has_optimistic_success_rate() {
        let w = SlidingWindow::new(5);
        assert_eq!(w.success_rate(), 1.0);
        assert!(w.is_empty());
    }

    #[test]
    fn all_success_yields_1() {
        let mut w = SlidingWindow::new(3);
        w.push(true);
        w.push(true);
        w.push(true);
        assert_eq!(w.success_rate(), 1.0);
    }

    #[test]
    fn mixed_results_yield_correct_rate() {
        let mut w = SlidingWindow::new(4);
        w.push(true);
        w.push(false);
        w.push(true);
        w.push(false);
        assert!((w.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn oldest_sample_is_evicted_when_full() {
        let mut w = SlidingWindow::new(3);
        w.push(false); // will be evicted
        w.push(true);
        w.push(true);
        w.push(true); // evicts the false
        assert_eq!(w.success_rate(), 1.0);
        assert_eq!(w.len(), 3);
    }

    #[test]
    fn all_failures_yield_0() {
        let mut w = SlidingWindow::new(3);
        w.push(false);
        w.push(false);
        assert_eq!(w.success_rate(), 0.0);
    }

    #[test]
    fn clear_resets_to_empty() {
        let mut w = SlidingWindow::new(3);
        w.push(true);
        w.push(false);
        w.clear();
        assert!(w.is_empty());
        assert_eq!(w.success_rate(), 1.0);
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn zero_capacity_panics() {
        let _w = SlidingWindow::new(0);
    }
}
