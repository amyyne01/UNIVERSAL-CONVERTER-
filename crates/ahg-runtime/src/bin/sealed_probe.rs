//! Release-binary probe for `tests/sealed.rs`: reveals the dummy pair and prints
//! only a fixed word, never a value. MARKER is a positive control for the scan.
#[allow(dead_code)]
mod fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sealed-dummy.rs"));
}

static MARKER: &str = "AHG-PROBE-MARKER-7f3a";

fn main() {
    std::hint::black_box(MARKER);
    let ok = ahg_runtime::sealed::reveal(&fixture::PAD, &fixture::MASKED).is_some();
    println!("{}", if ok { "ok" } else { "none" });
}
