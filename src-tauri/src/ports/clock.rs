//! A clock abstraction so time-dependent behaviour (auto-lock) can be tested
//! deterministically by injecting a fake clock.

pub trait Clock {
    /// Current time as whole seconds. Monotonic enough for inactivity timeouts.
    fn now_secs(&self) -> u64;
}
