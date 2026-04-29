//! Health checking subsystem (T1.0.2.04–08).
//!
//! * [`SlidingWindow`] — success-rate ring buffer
//! * [`HealthChecker`] — periodic probe loop + status cache + events

pub mod checker;
pub mod sliding_window;

pub use checker::{
    HealthChangedEvent, HealthChecker, HealthSnapshot, DEFAULT_CHECK_INTERVAL, DEFAULT_WINDOW_SIZE,
    DEGRADED_THRESHOLD, DOWN_THRESHOLD, EVENT_PROVIDER_STATUS_CHANGED,
};
pub use sliding_window::SlidingWindow;
