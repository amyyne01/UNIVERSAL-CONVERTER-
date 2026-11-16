//! Shared by the updater tests: the scripted network lives in the crate's
//! `standin` module, so there is one copy.
#![allow(dead_code)]
use sha2::{Digest, Sha256};

pub use ahg_runtime::standin::Net;

pub fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}
