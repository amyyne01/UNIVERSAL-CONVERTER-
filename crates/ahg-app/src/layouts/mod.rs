//! The three new layouts of the design gate (owner, 2026-11-15): the whole
//! app redrawn from scratch, with no sidebar and no pages. Each draws the
//! window from the same `gate::State` and wears one of the looks in
//! `directions/`, and each is behind its own cargo feature.
#[cfg(feature = "bar")]
pub mod bar;
#[cfg(feature = "collection")]
pub mod collection;
#[cfg(feature = "stage")]
pub mod stage;
