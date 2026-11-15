//! The three directions of the design gate. Each draws the whole window from
//! the same `gate::State`, and each is behind its own cargo feature.
#[cfg(feature = "orbit")]
pub mod orbit;
#[cfg(feature = "shelf")]
pub mod shelf;
#[cfg(feature = "signal")]
pub mod signal;
