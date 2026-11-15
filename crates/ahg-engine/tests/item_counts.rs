//! D31, an approved difference from Electron 1.6.0: a playlist download
//! records which item it is on, so the row can show "3/12".
use ahg_engine::downloader::progress::{OutputScan, playlist_item};

#[test]
fn the_item_line_is_read_as_index_and_total() {
    assert_eq!(playlist_item("[download] Downloading item 3 of 12"), Some((3.0, 12.0)));
    assert_eq!(playlist_item("[download] Downloading item 10 of 10\r"), Some((10.0, 10.0)));
    assert_eq!(playlist_item("[download] Downloading item 3"), None);
    assert_eq!(playlist_item("[download] Downloading item x of 12"), None);
    assert_eq!(playlist_item("[download] Downloading item 3 of"), None);
    assert_eq!(playlist_item(" [download] Downloading item 3 of 12"), None);
    assert_eq!(playlist_item("[download] Downloading playlist: Mix"), None);
}

#[test]
fn a_scan_that_counts_items_fills_them_in_and_emits_the_change() {
    let mut scan = OutputScan::new().with_item_counts();
    let events = scan.push_stderr("[download] Downloading item 3 of 12\n");
    assert_eq!(events.len(), 1);
    assert_eq!((events[0].playlist_index, events[0].playlist_total), (3.0, 12.0));
    // The numbers stay on later updates for the same item.
    let events = scan.push_stderr("[ExtractAudio] Destination: a.mp3\n");
    assert_eq!((events[0].playlist_index, events[0].playlist_total), (3.0, 12.0));
    let events = scan.push_stderr("[download] Downloading item 4 of 12\n");
    assert_eq!(events[0].playlist_index, 4.0);
}

#[test]
fn a_plain_scan_leaves_the_numbers_alone_as_electron_does() {
    let mut scan = OutputScan::new();
    assert!(scan.push_stderr("[download] Downloading item 3 of 12\n").is_empty());
    assert_eq!(scan.progress().playlist_index, 0.0);
}
