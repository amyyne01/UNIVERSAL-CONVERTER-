//! A check that lands while a download is running must not break the download
//! (Electron copies the release into a local before its first await).
use ahg_engine::updater::Updater;
use serde_json::json;

const EXE: &str = "https://github.com/amyyne01/UNIVERSAL-CONVERTER-/releases/download/v2.1.0/AHG-Converter.exe";
const SUMS: &str = "https://github.com/amyyne01/UNIVERSAL-CONVERTER-/releases/download/v2.1.0/SHA256SUMS.txt";

fn feed(tag: &str) -> serde_json::Value {
    json!({ "tag_name": tag, "assets": [{ "name": "AHG-Converter.exe", "browser_download_url": EXE }, { "name": "SHA256SUMS.txt", "browser_download_url": SUMS }] })
}

fn available() -> Updater {
    let mut u = Updater::new("C:\\updates");
    u.on_release_json(&feed("v2.1.0"), "2.0.0");
    u
}

fn sums() -> String {
    format!("{}  AHG-Converter.exe\n", "ab".repeat(32))
}

#[test]
fn an_up_to_date_check_during_a_download_does_not_stop_it() {
    let mut u = available();
    u.download_begin().unwrap();
    u.on_release_json(&feed("v2.0.0"), "2.0.0");
    assert!(u.available().is_none());
    u.download_sums(&sums()).unwrap();
    let stage = u.download_finish(&"ab".repeat(32)).unwrap();
    u.download_written(&stage);
    assert!(u.staged().unwrap().ends_with("AHG-Converter.exe"));
}

#[test]
fn a_second_download_during_the_first_refuses_instead_of_panicking() {
    let mut u = available();
    u.download_begin().unwrap();
    u.download_sums(&sums()).unwrap();
    u.download_begin().unwrap();
    assert!(u.download_finish(&"ab".repeat(32)).is_err());
}

#[test]
fn steps_out_of_order_refuse() {
    let mut u = Updater::new("C:\\updates");
    assert!(u.download_sums("x").is_err());
    assert!(u.download_finish("x").is_err());
}
