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
    assert_eq!(r.token(), TOKEN);
    assert_eq!(r.registry(), ID);
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
fn a_one_byte_tag_difference_at_each_position_gives_none() {
    let (pad, masked) = fixture::pair();
    for p in 128..136 {
        let mut m = masked;
        m[p] ^= 0x80;
        assert!(reveal(&pad, &m).is_none(), "tag byte {p}");
    }
}

#[test]
fn a_thousand_random_pairs_never_panic() {
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed as u8
    };
    for _ in 0..1000 {
        let (mut a, mut b) = ([0u8; 136], [0u8; 136]);
        for i in 0..136 {
            a[i] = next();
            b[i] = next();
        }
        assert!(reveal(&a, &b).is_none());
    }
}

#[test]
fn the_discord_id_is_a_plain_constant_in_the_fixture() {
    assert_eq!(fixture::DISCORD_CLIENT_ID, "000000000000000000");
}

#[test]
fn real_mode_reads_both_spellings_and_the_discord_id() {
    if !node_available() {
        eprintln!("SKIPPED: node not found");
        return;
    }
    // An ignored output path inside the repo; removed afterwards.
    let out = ignored_out(&format!("ahg-sealed-spell-{}", std::process::id()));
    let dir = temp_dir("spell");
    let cases = [
        ("a.env", "AHG_GITHUB_TOKEN=fake-tok\nAHG_LICENSE_GIST_ID=fake-gist\nAHG_DISCORD_CLIENT_ID=123456\n"),
        ("b.json", r#"{"githubToken":"fake-tok","licenseGistId":"fake-gist","discordClientId":"123456"}"#),
        ("c.json", r#"{"AHG_GITHUB_TOKEN":"fake-tok","AHG_LICENSE_GIST_ID":"fake-gist","AHG_DISCORD_CLIENT_ID":"123456"}"#),
    ];
    for (name, body) in cases {
        let f = dir.join(name);
        std::fs::write(&f, body).unwrap();
        let o = seal(&["--env", f.to_str().unwrap(), "--out", out.to_str().unwrap()]);
        let text = std::fs::read_to_string(&out);
        let _ = std::fs::remove_file(&out);
        assert!(o.status.success(), "{name}: {}", String::from_utf8_lossy(&o.stderr));
        let t = text.unwrap();
        let r = reveal(&parse_array(&t, "PAD"), &parse_array(&t, "MASKED")).expect("reveals");
        assert_eq!((r.token(), r.registry()), ("fake-tok", "fake-gist"), "{name}");
        assert!(t.contains("pub const DISCORD_CLIENT_ID: &str = \"123456\";"), "{name}");
    }
    cleanup(&dir);
}

#[test]
fn an_empty_token_or_id_gives_none() {
    let (pad, masked) = fixture::pair();
    let mut base = [0u8; 136];
    for i in 0..136 {
        base[i] = pad[i] ^ masked[i];
    }
    // [1][len][token][len][id]: build from the real record's layout.
    let tl = base[1] as usize;
    let il = base[2 + tl] as usize;
    let (tok, id) = (base[2..2 + tl].to_vec(), base[3 + tl..3 + tl + il].to_vec());
    for (t, i) in [(vec![], id.clone()), (tok.clone(), vec![]), (vec![], vec![])] {
        let mut plain = [7u8; 136];
        plain[0] = 1;
        plain[1] = t.len() as u8;
        plain[2..2 + t.len()].copy_from_slice(&t);
        plain[2 + t.len()] = i.len() as u8;
        plain[3 + t.len()..3 + t.len() + i.len()].copy_from_slice(&i);
        let m = reseal(&pad, &mut plain);
        assert!(reveal(&pad, &m).is_none(), "empty field ({} / {})", t.len(), i.len());
    }
}

/// A git-ignored output path inside the repo (`crates/**/secrets.gen.rs`), unique per tag.
fn ignored_out(tag: &str) -> PathBuf {
    let d = repo_root().join(format!("crates/ahg-runtime/sealtmp-{tag}"));
    std::fs::create_dir_all(&d).unwrap();
    d.join("secrets.gen.rs")
}

fn cleanup(dir: &Path) {
    let tag = dir.file_name().unwrap().to_str().unwrap();
    let _ = std::fs::remove_dir_all(repo_root().join(format!("crates/ahg-runtime/sealtmp-{tag}")));
    let _ = std::fs::remove_dir_all(dir);
}

fn run_with_env(dir: &Path, name: &str, body: &[u8], secret: &str) -> (std::process::Output, PathBuf) {
    let f = dir.join(name);
    std::fs::write(&f, body).unwrap();
    // An ignored output path inside the repo.
    let out = ignored_out(dir.file_name().unwrap().to_str().unwrap());
    let _ = std::fs::remove_file(&out);
    let o = seal(&["--env", f.to_str().unwrap(), "--out", out.to_str().unwrap()]);
    let (so, se) = (String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    assert!(!so.contains(secret) && !se.contains(secret), "{name}: secret on output");
    (o, out)
}

#[test]
fn a_bad_env_file_never_prints_its_content() {
    if !node_available() {
        eprintln!("SKIPPED: node not found");
        return;
    }
    let dir = temp_dir("bad");
    let secret = "fake-secret-value-xyz";
    let trailing = format!("{{\"githubToken\":\"{secret}\",\"licenseGistId\":\"fake-gist\",}}");
    let (o, out) = run_with_env(&dir, "trailing.json", trailing.as_bytes(), secret);
    assert!(!o.status.success() && !out.exists(), "trailing comma refused, nothing written");
    // A UTF-8 BOM: stripped, so it succeeds.
    let bom = format!("\u{feff}{{\"githubToken\":\"{secret}\",\"licenseGistId\":\"fake-gist\"}}");
    let (o, out) = run_with_env(&dir, "bom.json", bom.as_bytes(), secret);
    let wrote = out.exists();
    let _ = std::fs::remove_file(&out);
    assert!(o.status.success() && wrote, "BOM file seals");
    let (o, out) = run_with_env(&dir, "missing.json", b"", secret);
    assert!(!o.status.success() && !out.exists());
    cleanup(&dir);
}

#[test]
fn whitespace_or_hash_in_a_value_is_refused_naming_only_the_key() {
    if !node_available() {
        eprintln!("SKIPPED: node not found");
        return;
    }
    let dir = temp_dir("ws");
    let cases: [(&str, String); 4] = [
        ("a.env", "AHG_GITHUB_TOKEN=fake-sec ret\nAHG_LICENSE_GIST_ID=g\n".into()),
        ("b.env", "AHG_GITHUB_TOKEN=fake-sec#ret\nAHG_LICENSE_GIST_ID=g\n".into()),
        ("c.json", r#"{"githubToken":"fake-sec ret","licenseGistId":"g"}"#.into()),
        ("d.json", r#"{"githubToken":"fakesecret","licenseGistId":"g#x"}"#.into()),
    ];
    for (name, body) in &cases {
        let (o, out) = run_with_env(&dir, name, body.as_bytes(), "fake-sec");
        assert!(!o.status.success() && !out.exists(), "{name} refused");
        let said = String::from_utf8_lossy(&o.stderr);
        assert!(said.contains("AHG_") || said.contains("githubToken") || said.contains("licenseGistId"), "{name}: names the key");
    }
    cleanup(&dir);
}

#[test]
fn out_without_a_value_fails_and_leaves_the_fixture_alone() {
    if !node_available() {
        eprintln!("SKIPPED: node not found");
        return;
    }
    let fixture = repo_root().join("crates/ahg-runtime/tests/fixtures/sealed-dummy.rs");
    let before = std::fs::read(&fixture).unwrap();
    for args in [&["--dummy", "--out"][..], &["--dummy", "--out", "--dummy"][..]] {
        assert!(!seal(args).status.success(), "{args:?}");
    }
    assert_eq!(std::fs::read(&fixture).unwrap(), before);
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
        assert_eq!((r.token(), r.registry()), (TOKEN, ID));
    }
    cleanup(&dir);
}

#[test]
fn real_mode_refuses_a_path_that_is_not_git_ignored() {
    if !node_available() {
        eprintln!("SKIPPED: node not found");
        return;
    }
    let dir = temp_dir("real");
    let env = dir.join("fake.env");
    std::fs::write(&env, "AHG_GITHUB_TOKEN=fake-token-for-test\nAHG_LICENSE_GIST_ID=fake-id-for-test\n").unwrap();
    let json = dir.join("fake.json");
    std::fs::write(&json, r#"{"githubToken":"fake-token-for-test","licenseGistId":"fake-id-for-test"}"#).unwrap();
    // Outside any repo (git exits 128), and inside this repo but tracked-style (exit 1).
    let inside = repo_root().join("crates/ahg-runtime/tests/fixtures/not-ignored.rs");
    for (e, out) in [(&env, dir.join("out.rs")), (&json, inside)] {
        let o = seal(&["--env", e.to_str().unwrap(), "--out", out.to_str().unwrap()]);
        assert!(!o.status.success(), "must refuse {}", out.display());
        assert!(!out.exists(), "nothing written");
        let said = String::from_utf8_lossy(&o.stderr);
        assert!(!said.contains("fake-token-for-test") && !said.contains("fake-id-for-test"));
    }
    cleanup(&dir);
}

/// The built probe must not carry the dummy plaintext. Only the release build's
/// folding is meaningful, so a debug run skips (the positive control still runs).
#[test]
fn probe_binary_does_not_contain_the_plaintext() {
    let path = env!("CARGO_BIN_EXE_sealed_probe");
    let bin = std::fs::read(path).expect("read probe");
    assert!(!bin.is_empty(), "empty haystack");
    let has = |needle: &[u8]| bin.windows(needle.len()).any(|w| w == needle);
    assert!(has(b"AHG-PROBE-MARKER-7f3a"), "positive control missing: scan is blind");
    let out = Command::new(path).output().expect("run probe");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "ok");
    if cfg!(debug_assertions) {
        eprintln!("skipped: absence is only checked in a release build");
        return;
    }
    for s in [TOKEN, ID, fixture::DISCORD_CLIENT_ID] {
        let wide: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert!(!has(s.as_bytes()), "ASCII plaintext in binary");
        assert!(!has(&wide), "UTF-16LE plaintext in binary");
    }
}
