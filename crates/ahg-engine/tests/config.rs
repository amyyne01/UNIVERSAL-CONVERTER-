//! Compares the settings file handling with electron/config.ts. The golden
//! file (tests/golden/config.json, written by scripts/rust-golden.mjs) was
//! produced by driving the real ConfigManager in a scratch folder whose path is
//! recorded as `{TMP}`; these tests rebuild that layout in their own folder.
#![cfg(windows)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ahg_engine::atomic_write::write_file_atomic;
use ahg_engine::config::{ConfigManager, default_config, migrate_config};
use serde_json::{Map, Value, json};

fn golden() -> Value {
    serde_json::from_str(include_str!("golden/config.json")).unwrap()
}

/// A scratch folder laid out like the generator's: Downloads\, out\ and file.txt.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("ahg-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Downloads")).unwrap();
        fs::create_dir_all(dir.join("out")).unwrap();
        fs::write(dir.join("file.txt"), "x").unwrap();
        Self(dir)
    }
    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
    fn downloads(&self) -> String {
        format!("{}\\Downloads", self.path())
    }
    /// `{TMP}` replaced by this folder, throughout a JSON value.
    fn real(&self, v: &Value) -> Value {
        match v {
            Value::String(s) => Value::String(s.replace("{TMP}", self.path())),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.real(x)).collect()),
            Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), self.real(x))).collect()),
            other => other.clone(),
        }
    }
    /// The same inside JSON text, where backslashes are doubled.
    fn real_text(&self, text: &str) -> String {
        text.replace("{TMP}", &self.path().replace('\\', "\\\\"))
    }
    fn tok_text(&self, text: &str) -> String {
        text.replace(&self.path().replace('\\', "\\\\"), "{TMP}")
    }
    fn manager(&self, user_data: &Path) -> ConfigManager {
        ConfigManager::new(user_data, self.downloads())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn defaults_match_electron() {
    let s = Scratch::new("defaults");
    let c = s.manager(&s.0.join("u"));
    assert_eq!(Value::Object(c.get_all().clone()), s.real(&golden()["defaults"]));
    assert_eq!(default_config().len(), 36);
}

#[test]
fn set_accepts_and_rejects_what_electron_does() {
    let s = Scratch::new("set");
    let cases = golden()["set"].as_array().unwrap().clone();
    assert!(cases.len() > 600);
    let mut wrong = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        let mut c = s.manager(&s.0.join("u").join(i.to_string()));
        let key = case["key"].as_str().unwrap();
        let ok = c.set(key, s.real(&case["value"]));
        let after = c.get(key).cloned();
        let expected_after = case.get("after").map(|v| s.real(v));
        if json!(ok) != case["ok"] || after != expected_after {
            wrong.push(format!("{case}\n  rust: ok={ok} after={after:?}"));
        }
    }
    assert!(wrong.is_empty(), "{} mismatches:\n{}", wrong.len(), wrong.join("\n"));
    // Rust only: a key lookup is a map lookup, never the prototype chain.
    assert_eq!(s.manager(&s.0.join("u").join("proto")).get("__proto__"), None);
}

#[test]
fn load_reads_merges_and_restores_as_electron_does() {
    let s = Scratch::new("load");
    let cases = golden()["load"].as_array().unwrap().clone();
    assert!(cases.len() >= 30);
    for (i, case) in cases.iter().enumerate() {
        let dir = s.0.join("l").join(i.to_string());
        fs::create_dir_all(&dir).unwrap();
        if let Some(file) = case["file"].as_str() {
            fs::write(dir.join("config.json"), s.real_text(file)).unwrap();
        }
        for (n, backup) in case["backups"].as_array().unwrap().iter().enumerate() {
            fs::write(dir.join(format!("config.backup.{}.json", n + 1)), s.real_text(backup.as_str().unwrap())).unwrap();
        }
        let c = s.manager(&dir);
        assert_eq!(Value::Object(c.get_all().clone()), s.real(&case["out"]), "case {i}: {case}");
        assert_eq!(json!(c.was_restored_from_backup()), case["restored"], "case {i}: {case}");
    }
}

#[test]
fn every_write_puts_the_same_bytes_on_disk_as_electron() {
    let s = Scratch::new("write");
    let dir = s.0.join("w");
    fs::create_dir_all(&dir).unwrap();
    let mut c = s.manager(&dir);
    let read = |name: &str| fs::read_to_string(dir.join(name)).ok().map(|t| s.tok_text(&t));
    let g = golden();
    let steps = g["write"].as_array().unwrap();
    assert!(!steps.is_empty(), "config.json has no write steps");
    for (i, step) in steps.iter().enumerate() {
        c.update(s.real(&step["update"]).as_object().unwrap());
        c.save_now();
        let after = json!({
            "config": read("config.json"),
            "backup1": read("config.backup.1.json"),
            "backup2": read("config.backup.2.json"),
            "backup3": read("config.backup.3.json"),
            "tmp": dir.join("config.json.tmp").exists(),
        });
        assert_eq!(after, step["after"], "step {i}");
    }
}

#[test]
fn migrate_config_stamps_the_schema_version_as_electron_does() {
    let g = golden();
    let cases = g["migrate"].as_array().unwrap();
    assert!(!cases.is_empty(), "config.json has no migrate cases");
    for case in cases {
        let out = migrate_config(case["obj"].as_object().unwrap().clone());
        assert_eq!(Value::Object(out), case["out"], "{case}");
    }
}

/// Electron crashes at launch on these hand-edited values; Rust stamps them.
#[test]
fn a_fractional_or_negative_schema_version_is_stamped_instead_of_crashing() {
    for v in [json!(-1), json!(0.5)] {
        let mut obj = Map::new();
        obj.insert("schemaVersion".into(), v);
        assert_eq!(migrate_config(obj)["schemaVersion"], json!(1));
    }
}

#[test]
fn set_writes_are_debounced_by_400_ms() {
    let s = Scratch::new("debounce");
    let dir = s.0.join("d");
    fs::create_dir_all(&dir).unwrap();
    let mut c = s.manager(&dir);
    let t0 = Instant::now();
    assert!(c.save_deadline().is_none());
    assert!(c.set_at("theme", json!("dark"), t0));
    c.poll_at(t0 + Duration::from_millis(399));
    assert!(!dir.join("config.json").exists(), "written before the debounce elapsed");
    // A second change restarts the wait.
    assert!(c.set_at("palette", json!("moss"), t0 + Duration::from_millis(399)));
    c.poll_at(t0 + Duration::from_millis(798));
    assert!(!dir.join("config.json").exists());
    assert_eq!(c.save_deadline(), Some(t0 + Duration::from_millis(799)));
    c.poll_at(t0 + Duration::from_millis(799));
    let saved: Value = serde_json::from_str(&fs::read_to_string(dir.join("config.json")).unwrap()).unwrap();
    assert_eq!(saved["theme"], "dark");
    assert_eq!(saved["palette"], "moss");
    assert!(c.save_deadline().is_none());
}

#[test]
fn on_change_fires_after_set_and_update_until_unsubscribed() {
    let s = Scratch::new("listeners");
    let mut c = s.manager(&s.0.join("u"));
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let id = c.on_change(move || {
        seen.fetch_add(1, Ordering::SeqCst);
    });
    assert!(c.set("theme", json!("dark")));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    c.update(json!({ "theme": "light" }).as_object().unwrap());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    // A refused set changes nothing and notifies nobody; update always notifies.
    assert!(!c.set("theme", json!(5)));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    c.update(json!({ "nope": 1 }).as_object().unwrap());
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    c.off_change(id);
    assert!(c.set("theme", json!("dark")));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn listeners_run_in_the_order_they_were_added() {
    let s = Scratch::new("order");
    let mut c = s.manager(&s.0.join("u"));
    let log = Arc::new(Mutex::new(Vec::new()));
    for name in ["first", "second"] {
        let log = log.clone();
        c.on_change(move || log.lock().unwrap().push(name));
    }
    c.set("theme", json!("dark"));
    assert_eq!(*log.lock().unwrap(), ["first", "second"]);
}

// ── atomic-write.test.ts ─────────────────────────────────────────────────────

#[test]
fn write_file_atomic_writes_the_file_and_leaves_no_tmp_behind() {
    let s = Scratch::new("atomic1");
    let target = s.0.join("a.json");
    write_file_atomic(&target, "{\"a\":1}").unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "{\"a\":1}");
    assert!(!s.0.join("a.json.tmp").exists());
}

#[test]
fn write_file_atomic_overwrites_an_existing_file() {
    let s = Scratch::new("atomic2");
    let target = s.0.join("a.json");
    fs::write(&target, "old").unwrap();
    write_file_atomic(&target, "new").unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "new");
}

#[test]
fn write_file_atomic_keeps_the_original_and_cleans_tmp_when_the_swap_fails() {
    let s = Scratch::new("atomic3");
    // A folder with something in it cannot be replaced by a file, so the rename fails.
    let target = s.0.join("out");
    fs::write(target.join("keep.txt"), "original").unwrap();
    assert!(write_file_atomic(&target, "new").is_err());
    assert_eq!(fs::read_to_string(target.join("keep.txt")).unwrap(), "original");
    assert!(!s.0.join("out.tmp").exists());
}
