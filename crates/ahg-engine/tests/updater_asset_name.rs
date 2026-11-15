//! D32, an approved difference from Electron 1.6.0 (and to be backported): the
//! updater saves a download under the release asset's name, so that name must
//! be a plain file name. Electron uses it as given, which lets a name like
//! `..\evil.exe` be written outside the updates folder.
use ahg_engine::updater::{Updater, is_plain_file_name};
use serde_json::json;

#[test]
fn only_a_plain_file_name_is_accepted() {
    for ok in ["AHG-Converter.exe", "AHG Converter 2.0.exe", "a.b.c.exe", "\u{e9}t\u{e9}.exe"] {
        assert!(is_plain_file_name(ok), "{ok}");
    }
    for bad in [
        "",
        ".",
        "..",
        r"..\evil.exe",
        "../evil.exe",
        r"sub\app.exe",
        "sub/app.exe",
        "C:evil.exe",
        r"C:\Windows\evil.exe",
        r"\\server\share\evil.exe",
        "evil.exe:stream",
        "a\0b.exe",
        "a\nb.exe",
        "app.exe.",
        "app.exe ",
        "CON.exe",
        "nul.exe",
        "COM1.exe",
    ] {
        assert!(!is_plain_file_name(bad), "{bad:?}");
    }
}

fn release(asset_name: &str) -> serde_json::Value {
    json!({
        "tag_name": "v9.9.9",
        "assets": [
            { "name": asset_name, "browser_download_url": "https://github.com/o/r/releases/download/v9.9.9/app.exe" },
            { "name": "SHA256SUMS.txt", "browser_download_url": "https://github.com/o/r/releases/download/v9.9.9/SHA256SUMS.txt" },
        ]
    })
}

#[test]
fn a_release_whose_asset_name_is_a_path_is_refused_as_an_error() {
    let dir = std::env::temp_dir().join(format!("ahg-d32-{}", std::process::id()));
    let dir = dir.to_str().unwrap();
    let mut updater = Updater::new(dir);
    updater.begin_check();
    let events = updater.on_release_json(&release(r"..\evil.exe"), "1.0.0");
    assert!(events.iter().any(|e| e.channel == "update:error"), "{events:?}");
    assert!(!events.iter().any(|e| e.channel == "update:available"));
    assert!(updater.download_begin().is_err(), "nothing may be downloaded for it");

    let mut updater = Updater::new(dir);
    updater.begin_check();
    let events = updater.on_release_json(&release("AHG-Converter.exe"), "1.0.0");
    assert!(events.iter().any(|e| e.channel == "update:available"), "{events:?}");
}
