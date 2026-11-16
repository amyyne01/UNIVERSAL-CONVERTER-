//! Reveals the licence client's credentials, sealed into the EXE at build time by
//! `scripts/seal-secrets.mjs`. `PAD` and `MASKED` must be `static` and are read
//! volatile: as plain `const`s a release build folded the plaintext back in.
use core::fmt;
use core::ptr::read_volatile;

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub struct Revealed {
    pub token: Zeroizing<String>,
    pub registry: Zeroizing<String>,
}

impl fmt::Debug for Revealed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Revealed(..)")
    }
}

/// Unmask, check the tag and the version, and read the two fields. Any problem is `None`.
#[inline(never)]
pub fn reveal(pad: &[u8; 136], masked: &[u8; 136]) -> Option<Revealed> {
    let mut buf = Zeroizing::new([0u8; 136]);
    for i in 0..136 {
        // SAFETY: `i` is in bounds of both arrays.
        buf[i] = unsafe { read_volatile(pad.as_ptr().add(i)) ^ read_volatile(masked.as_ptr().add(i)) };
    }
    if Sha256::digest(&buf[..128])[..8] != buf[128..] || buf[0] != 1 {
        return None;
    }
    let (token, rest) = field(&buf[1..128])?;
    let (registry, _) = field(rest)?;
    Some(Revealed { token, registry })
}

/// A length byte, then that many UTF-8 bytes; returns the text and the rest.
fn field(b: &[u8]) -> Option<(Zeroizing<String>, &[u8])> {
    let (&n, rest) = b.split_first()?;
    let (s, rest) = rest.split_at_checked(n as usize)?;
    Some((Zeroizing::new(core::str::from_utf8(s).ok()?.to_owned()), rest))
}
