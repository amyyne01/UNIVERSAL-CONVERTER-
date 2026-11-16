//! `sealed::reveal` and the build-time sealer script, with dummy values only.
use std::path::{Path, PathBuf};
use std::process::Command;

use ahg_runtime::sealed::reveal;
use sha2::{Digest, Sha256};

mod fixture {
    include!("fixtures/sealed-dummy.rs");
    pub fn pair() -> ([u8; 136], [u8; 136]) {
        (PAD, MASKED)
    }
}

const TOKEN: &str = "dummy-token-0000-not-a-secret";
const ID: &str = "dummy-id-1111-not-a-secret";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Mask `plain` with `pad`, after setting a valid tag over its first 128 bytes.
fn reseal(pad: &[u8; 136], plain: &mut [u8; 136]) -> [u8; 136] {
    let tag = Sha256::digest(&plain[..128]);
    plain[128..].copy_from_slice(&tag[..8]);
    let mut m = [0u8; 136];
    for i in 0..136 {
        m[i] = plain[i] ^ pad[i];
    }
    m
}

#[test]
fn round_trip_of_the_tracked_dummy_fixture() {
    let (pad, masked) = fixture::pair();
    let r = reveal(&pad, &masked).expect("fixture reveals");
    assert_eq!(&*r.token, TOKEN);
    assert_eq!(&*r.registry, ID);
}

#[test]
fn damaged_input_gives_none() {
    let (pad, masked) = fixture::pair();
    let positions = [0usize, 1, 2, 60, 127, 128, 131, 135];
    for p in positions {
        let mut m = masked;
        m[p] ^= 0x01;
        assert!(reveal(&pad, &m).is_none(), "masked byte {p}");
        let mut q = pad;
        q[p] ^= 0x01;
        assert!(reveal(&q, &masked).is_none(), "pad byte {p}");
    }
    // Wrong version with a valid tag.
    let mut plain = [0u8; 136];
    for i in 0..136 {
        plain[i] = pad[i] ^ masked[i];
    }
    plain[0] = 2;
    let m = reseal(&pad, &mut plain);
    assert!(reveal(&pad, &m).is_none(), "wrong version");
    // A field length that runs past the record, with a valid tag.
    plain[0] = 1;
    plain[1] = 250;
    let m = reseal(&pad, &mut plain);
    assert!(reveal(&pad, &m).is_none(), "token length too long");
    // Not UTF-8, with a valid tag.
    plain[1] = 2;
    plain[2] = 0xff;
    plain[3] = 0xfe;
    let m = reseal(&pad, &mut plain);
    assert!(reveal(&pad, &m).is_none(), "not utf-8");
    // All zero, and random bytes.
    assert!(reveal(&[0; 136], &[0; 136]).is_none());
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..32 {
        let mut a = [0u8; 136];
        let mut b = [0u8; 136];
        for i in 0..136 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            a[i] = seed as u8;
            b[i] = (seed >> 8) as u8;
        }
        assert!(reveal(&a, &b).is_none());
    }
}

#[test]
fn debug_prints_no_content() {
    let (pad, masked) = fixture::pair();
    let r = reveal(&pad, &masked).unwrap();
    let s = format!("{r:?}");
    assert_eq!(s, "Revealed(..)");
    assert!(!s.contains(TOKEN) && !s.contains(ID));
}

fn node_available() -> bool {
    Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ahg-sealed-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn seal(args: &[&str]) -> std::process::Output {
    Command::new("node")
        .arg("scripts/seal-secrets.mjs")
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("run node")
}

/// The numbers inside the `[...]` that follows `pub static <name>`.
fn parse_array(text: &str, name: &str) -> [u8; 136] {
    let at = text.find(&format!("pub static {name}")).expect("array present");
    let rest = &text[at..];
    let body = &rest[rest.find("= [").unwrap() + 3..];
    let body = &body[..body.find(']').unwrap()];
    let v: Vec<u8> = body
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| u8::from_str_radix(s.trim_start_matches("0x"), 16).unwrap())
        .collect();
    v.try_into().expect("136 bytes")
}

#[test]
fn script_writes_fresh_pads_that_reveal_the_dummy() {
    if !node_available() {
        eprintln!("SKIPPED: node not found");
        return;
    }
    let dir = temp_dir("dummy");
    let (a, b) = (dir.join("a.rs"), dir.join("b.rs"));
    for f in [&a, &b] {
        let o = seal(&["--dummy", "--out", f.to_str().unwrap()]);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }
    let (ta, tb) = (std::fs::read_to_string(&a).unwrap(), std::fs::read_to_string(&b).unwrap());
    assert_ne!(ta, tb, "a fresh pad each run");
    for t in [&ta, &tb] {
        let r = reveal(&parse_array(t, "PAD"), &parse_array(t, "MASKED")).expect("reveals");
        assert_eq!((r.token.as_str(), r.registry.as_str()), (TOKEN, ID));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn real_mode_refuses_a_path_that_is_not_git_ignored() {
    if !node_available() {
        eprintln!("SKIPPED: node not found");
        return;
    }
    let dir = temp_dir("real");
    let env = dir.join("fake.env");
    std::fs::write(&env, "LICENSE_TOKEN=fake-token-for-test\nLICENSE_REGISTRY_ID=fake-id-for-test\n").unwrap();
    let json = dir.join("fake.json");
    std::fs::write(&json, r#"{"LICENSE_TOKEN":"fake-token-for-test","LICENSE_REGISTRY_ID":"fake-id-for-test"}"#).unwrap();
    // Outside any repo (git exits 128), and inside this repo but tracked-style (exit 1).
    let inside = repo_root().join("crates/ahg-runtime/tests/fixtures/not-ignored.rs");
    for (e, out) in [(&env, dir.join("out.rs")), (&json, inside)] {
        let o = seal(&["--env", e.to_str().unwrap(), "--out", out.to_str().unwrap()]);
        assert!(!o.status.success(), "must refuse {}", out.display());
        assert!(!out.exists(), "nothing written");
        let said = String::from_utf8_lossy(&o.stderr);
        assert!(!said.contains("fake-token-for-test") && !said.contains("fake-id-for-test"));
    }
    let _ = std::fs::remove_dir_all(dir);
}
