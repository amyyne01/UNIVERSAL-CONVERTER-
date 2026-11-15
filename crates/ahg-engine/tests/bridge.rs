//! The Spotify bridge's matching rules against their reference,
//! tools/match-model/match.mjs (tests/golden/bridge.json, written by
//! tools/match-model/golden.mjs from collected songs and real search responses).
use ahg_engine::downloader::bridge::{Lookup, Verdict, norm, verify};
use ahg_engine::types::Track;
use serde_json::{Value, json};

fn golden() -> Value {
    serde_json::from_str(include_str!("golden/bridge.json")).unwrap()
}

fn track(v: &Value) -> Track {
    let mut full = json!({ "id": "", "album": "", "trackNumber": 0, "thumbnailUrl": "" });
    full.as_object_mut().unwrap().extend(v.as_object().unwrap().clone());
    serde_json::from_value(full).unwrap()
}

#[test]
fn text_is_normalised_as_in_the_reference() {
    let golden = golden();
    let cases = golden["norm"].as_array().unwrap();
    assert!(cases.len() > 100, "bridge.json has too few texts");
    for c in cases {
        assert_eq!(norm(c["input"].as_str().unwrap()), c["output"].as_str().unwrap(), "norm of {}", c["input"]);
    }
}

#[test]
fn a_looked_up_video_gets_the_same_verdict_as_in_the_reference() {
    let golden = golden();
    let cases = golden["verdicts"].as_array().unwrap();
    assert!(cases.len() > 500, "bridge.json has too few verdicts");
    let mut seen = [0usize; 3];
    for c in cases {
        let look: Lookup = serde_json::from_value(c["look"].clone()).unwrap();
        let got = verify(&track(&c["track"]), &look);
        let name = match got {
            Verdict::Confirmed => "confirmed",
            Verdict::Rejected => "rejected",
            Verdict::Unknown => "unknown",
        };
        assert_eq!(name, c["verdict"].as_str().unwrap(), "{} against {}", c["track"], c["look"]);
        seen[got as usize] += 1;
    }
    assert!(seen.iter().all(|n| *n > 50), "every verdict should be well represented: {seen:?}");
}

/// The few-seconds rule, spelled out: the same recording may differ by a few
/// seconds between the two services, but a version note YouTube adds on its
/// own means a different cut.
#[test]
fn a_few_seconds_apart_is_confirmed_only_without_a_version_note_of_its_own() {
    let wanted = track(&json!({ "name": "I Wanna Rock", "artist": "Twisted Sister", "artists": ["Twisted Sister"], "durationMs": 180_000 }));
    let look = |title: &str, duration: f64| Lookup { duration: Some(duration), track: Some(title.into()), artist: Some("Twisted Sister".into()), creator: None };
    assert_eq!(verify(&wanted, &look("I Wanna Rock", 182.0)), Verdict::Confirmed);
    assert_eq!(verify(&wanted, &look("I Wanna Rock", 184.0)), Verdict::Confirmed);
    assert_eq!(verify(&wanted, &look("I Wanna Rock (2016 Remaster)", 184.0)), Verdict::Unknown);
    assert_eq!(verify(&wanted, &look("I Wanna Rock", 187.0)), Verdict::Unknown);
    assert_eq!(verify(&wanted, &look("I Wanna Rock", 200.0)), Verdict::Rejected);
    assert_eq!(verify(&wanted, &look("We're Not Gonna Take It", 219.0)), Verdict::Rejected);
    // No music metadata: nothing to judge by.
    assert_eq!(verify(&wanted, &Lookup { duration: Some(180.0), ..Default::default() }), Verdict::Unknown);
}

#[test]
fn a_song_search_row_agrees_or_not_as_in_the_reference() {
    use ahg_engine::downloader::bridge::{SongRow, row_agrees};
    let golden = golden();
    let cases = golden["rows"].as_array().unwrap();
    assert!(cases.len() > 500, "bridge.json has too few search rows");
    let mut yes = 0;
    for c in cases {
        let row = SongRow { title: c["row"]["title"].as_str().unwrap(), artist: c["row"]["artist"].as_str().unwrap_or(""), duration: c["row"]["duration"].as_f64() };
        let got = row_agrees(&track(&c["track"]), &row);
        assert_eq!(got, c["agrees"].as_bool().unwrap(), "{} against {}", c["track"], c["row"]);
        yes += usize::from(got);
    }
    assert!(yes > 100 && yes < cases.len() - 100, "both answers should be well represented: {yes} of {}", cases.len());
}

/// A composer is shared by every performer of a piece, so one shared name
/// does not make two credits the same.
#[test]
fn a_shared_composer_is_not_the_same_performer() {
    use ahg_engine::downloader::bridge::{SongRow, row_agrees};
    let wanted = track(&json!({ "name": "Prelude in C Major, BWV 846", "artist": "Johann Sebastian Bach, Lang Lang", "artists": ["Johann Sebastian Bach, Lang Lang"], "durationMs": 139_000 }));
    let row = |artist| SongRow { title: "Prelude in C Major, BWV 846", artist, duration: Some(139.0) };
    assert!(!row_agrees(&wanted, &row("Aurélien Pontier & Johann Sebastian Bach")));
    assert!(row_agrees(&wanted, &row("Lang Lang & Johann Sebastian Bach")));
    assert!(row_agrees(&wanted, &row("Lang Lang")), "YouTube may credit only one of them");
    // A one-word name must match whole.
    let nas = track(&json!({ "name": "Rodeo", "artist": "Nas", "artists": ["Nas"], "durationMs": 139_000 }));
    assert!(!row_agrees(&nas, &SongRow { title: "Rodeo", artist: "Lil Nas X", duration: Some(139.0) }));
}

#[test]
fn a_song_search_is_asked_and_read_as_in_the_reference() {
    use ahg_engine::downloader::bridge::{SONG_SEARCH_URL, song_rows, song_search_body};
    let golden = golden();
    assert_eq!(SONG_SEARCH_URL, golden["searchUrl"].as_str().unwrap());
    let cases = golden["searches"].as_array().unwrap();
    assert!(cases.len() >= 5, "bridge.json has too few searches");
    let mut explicit = 0;
    for c in cases {
        let body: Value = serde_json::from_str(&song_search_body(c["query"].as_str().unwrap())).unwrap();
        assert_eq!(body, c["body"], "request body for {}", c["query"]);
        let rows = song_rows(&c["response"].to_string());
        let got: Vec<Value> = rows
            .iter()
            .map(|r| json!({ "id": r.id, "title": r.title, "artist": r.artist, "album": r.album, "duration": r.duration, "explicit": r.explicit }))
            .collect();
        let want: Vec<Value> = c["rows"].as_array().unwrap().iter().map(|w| json!({ "id": w["id"], "title": w["title"], "artist": w["artist"], "album": w["album"], "duration": w["duration"].as_f64(), "explicit": w["explicit"] })).collect();
        assert!(!got.is_empty(), "no rows read for {}", c["query"]);
        assert_eq!(got, want, "rows for {}", c["query"]);
        explicit += rows.iter().filter(|r| r.explicit).count();
    }
    assert!(explicit > 0, "the cases should include explicit songs");
    // Anything that is not a search response gives no rows, and no panic.
    assert!(song_rows("").is_empty() && song_rows("[1, {\"a\": null}]").is_empty() && song_rows("<html>").is_empty());
}

#[test]
fn the_row_to_download_is_chosen_as_in_the_reference() {
    use ahg_engine::downloader::bridge::{SongHit, pick_row};
    let golden = golden();
    let cases = golden["picks"].as_array().unwrap();
    assert!(cases.len() > 300, "bridge.json has too few picks");
    let (mut found, mut differs) = (0, 0);
    for c in cases {
        let rows: Vec<SongHit> = c["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| SongHit {
                id: r["id"].as_str().unwrap().into(),
                title: r["title"].as_str().unwrap().into(),
                artist: r["artist"].as_str().unwrap_or("").into(),
                album: r["album"].as_str().unwrap_or("").into(),
                duration: r["duration"].as_f64(),
                explicit: r["explicit"].as_bool().unwrap(),
            })
            .collect();
        let got = pick_row(&track(&c["track"]), &rows, c["explicit"].as_bool());
        let got = got.map_or(Value::Null, |p| json!({ "index": p.index, "explicitAgrees": p.explicit_agrees }));
        assert_eq!(got, c["pick"], "{} with explicit {}", c["track"], c["explicit"]);
        found += usize::from(!got.is_null());
        differs += usize::from(got["explicitAgrees"] == false);
    }
    assert!(found > 200 && found < cases.len() && differs > 0, "found {found} of {}, {differs} with a different flag", cases.len());
}

/// "Needs review": the refusal is our own message. It must not be retried,
/// and the row shows what to do about it.
#[test]
fn a_track_held_for_review_is_not_retried_and_says_what_to_do() {
    use ahg_engine::downloader::bridge::NO_CONFIRMED_MATCH;
    use ahg_engine::downloader::errors::{ErrorClass, classify_error, explain_error};
    assert_eq!(classify_error(NO_CONFIRMED_MATCH), ErrorClass::Permanent);
    use ahg_engine::downloader::bridge::NO_MATCH_FOUND;
    assert_eq!(classify_error(NO_MATCH_FOUND), ErrorClass::Permanent);
    assert!(explain_error(NO_MATCH_FOUND).starts_with("No match was found for this track"), "{}", explain_error(NO_MATCH_FOUND));
    assert!(explain_error(NO_CONFIRMED_MATCH).ends_with("pick a match to download it."), "{}", explain_error(NO_CONFIRMED_MATCH));
}

// ── The file is named and tagged from Spotify's data ────────────────────────

fn spotify_args(details: Value, title: &str, embed_metadata: bool) -> Vec<String> {
    use ahg_engine::downloader::args::build_ytdlp_args;
    use ahg_engine::downloader::bridge::name_and_tag_from_spotify;
    use ahg_engine::queue::{TaskInit, create_task};
    use ahg_engine::types::SourcePlatform;
    let init = TaskInit {
        url: Some("https://open.spotify.com/track/abc".into()),
        source: Some(SourcePlatform::Spotify),
        output_dir: Some("C:\\Music".into()),
        format: Some("mp3".into()),
        is_audio_only: Some(true),
        embed_metadata: Some(embed_metadata),
        title: (!title.is_empty()).then(|| title.to_owned()),
        track: Some(track(&track_json(details))),
        ..Default::default()
    };
    let task = create_task(&init, "task_1".into(), 0);
    let mut args = build_ytdlp_args(&task, None);
    name_and_tag_from_spotify(&task, &mut args);
    args
}

fn track_json(extra: Value) -> Value {
    let mut t = json!({ "name": "Dreams - 2004 Remaster", "artist": "Fleetwood Mac", "artists": ["Fleetwood Mac"], "durationMs": 257800, "album": "Rumours", "albumYear": "1977", "trackNumber": 2 });
    t.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    t
}

/// The values given to `--parse-metadata`, in order.
fn tags(args: &[String]) -> Vec<&str> {
    args.windows(2).filter(|w| w[0] == "--parse-metadata").map(|w| w[1].as_str()).collect()
}

fn output(args: &[String]) -> &str {
    args.windows(2).find(|w| w[0] == "-o").map(|w| w[1].as_str()).unwrap()
}

#[test]
fn a_spotify_download_is_named_and_tagged_from_the_spotify_track() {
    let args = spotify_args(json!({}), "", true);
    assert_eq!(
        tags(&args),
        [
            "Dreams - 2004 Remaster:%(meta_title)s",
            "Fleetwood Mac:%(meta_artist)s",
            "Rumours:%(meta_album)s",
            "2:%(meta_track)s",
            "1977:%(meta_date)s",
        ]
    );
    assert_eq!(output(&args), "C:\\Music\\Fleetwood Mac - Dreams - 2004 Remaster.%(ext)s");
    // The link is still the last argument: the bridge replaces it with the match.
    assert_eq!(args.last().unwrap(), "https://open.spotify.com/track/abc");
    // Everything after "--" is a link to yt-dlp, so the tags must come before it.
    let end_of_options = args.iter().position(|a| a == "--").expect("the options end with --");
    assert_eq!(end_of_options, args.len() - 2);
    assert!(args.iter().rposition(|a| a == "--parse-metadata").unwrap() < end_of_options);
}

#[test]
fn every_artist_is_tagged_and_missing_details_are_left_to_youtube() {
    let args = spotify_args(json!({ "artist": "Taylor Swift", "artists": ["Taylor Swift", "Bon Iver"], "name": "exile (feat. Bon Iver)", "album": "", "albumYear": null, "trackNumber": 0 }), "", true);
    assert_eq!(tags(&args), ["exile (feat. Bon Iver):%(meta_title)s", "Taylor Swift, Bon Iver:%(meta_artist)s"]);
    assert_eq!(output(&args), "C:\\Music\\Taylor Swift - exile (feat. Bon Iver).%(ext)s");
}

/// The text is Spotify's, and yt-dlp reads these values in a small language
/// of its own: ":" separates, "%(...)s" is filled in. Measured against the
/// real yt-dlp: doubling "%" and escaping ":" carries any text through, except
/// a trailing backslash, which would swallow the separator.
#[test]
fn text_that_yt_dlp_would_read_as_its_own_language_is_made_plain() {
    let args = spotify_args(json!({ "name": "100% %(id)s: Live\\", "artist": "AC/DC", "artists": ["AC/DC"], "album": "A: B", "albumYear": null, "trackNumber": 0 }), "", true);
    assert_eq!(tags(&args), ["100%% %%(id)s\\: Live:%(meta_title)s", "AC/DC:%(meta_artist)s", "A\\: B:%(meta_album)s"]);
    // In a file name the characters Windows forbids go, and "%" is doubled so yt-dlp fills nothing in.
    assert_eq!(output(&args), "C:\\Music\\ACDC - 100%% %%(id)s Live.%(ext)s");
}

#[test]
fn a_name_the_request_chose_is_kept_and_tags_follow_the_embed_setting() {
    let named = spotify_args(json!({}), "My Own Name", true);
    assert_eq!(output(&named), "C:\\Music\\My Own Name.%(ext)s");
    assert_eq!(tags(&named).len(), 5);
    let untagged = spotify_args(json!({}), "", false);
    assert!(tags(&untagged).is_empty(), "no tags are written when embedding is off");
    assert_eq!(output(&untagged), "C:\\Music\\Fleetwood Mac - Dreams - 2004 Remaster.%(ext)s");
}

/// The same recording often sits on several albums (the original, a deluxe
/// edition, a best-of). When Spotify names the album, that one is taken.
#[test]
fn among_songs_that_agree_the_one_from_the_same_album_is_taken() {
    use ahg_engine::downloader::bridge::{SongHit, pick_row};
    let hit = |id: &str, album: &str, explicit: bool| SongHit { id: id.into(), title: "Dreams".into(), artist: "Fleetwood Mac".into(), album: album.into(), duration: Some(258.0), explicit };
    let wanted = |album: &str| track(&json!({ "name": "Dreams", "artist": "Fleetwood Mac", "artists": ["Fleetwood Mac"], "album": album, "durationMs": 257_800 }));
    let rows = [hit("deluxe", "Rumours (Deluxe Edition)", false), hit("bestof", "Greatest Hits", false), hit("original", "Rumours", false)];

    assert_eq!(pick_row(&wanted("Rumours"), &rows, None).unwrap().index, 2);
    assert_eq!(pick_row(&wanted("RUMOURS!"), &rows, None).unwrap().index, 2, "case and punctuation do not matter");
    assert_eq!(pick_row(&wanted("Greatest Hits"), &rows, None).unwrap().index, 1);
    // No album named, or one YouTube does not list: the first that agrees, as before.
    assert_eq!(pick_row(&wanted(""), &rows, None).unwrap().index, 0);
    assert_eq!(pick_row(&wanted("Live in Boston"), &rows, None).unwrap().index, 0);

    // The explicit flag outranks the album: the right version first, then the right edition.
    let mixed = [hit("clean-same-album", "Rumours", false), hit("explicit-other-album", "Greatest Hits", true)];
    let pick = pick_row(&wanted("Rumours"), &mixed, Some(true)).unwrap();
    assert_eq!((pick.index, pick.explicit_agrees), (1, true));
}

/// The file itself says which video it came from, and whether that was a
/// guess: once it is on disk the app's own mark is no longer beside it.
#[test]
fn the_file_notes_the_video_it_came_from_and_whether_that_was_a_guess() {
    use ahg_engine::downloader::bridge::note_the_source;
    let mut confirmed = spotify_args(json!({}), "", true);
    note_the_source(&mut confirmed, 1.0);
    // yt-dlp fills in the video's own title and address.
    assert_eq!(tags(&confirmed).last().copied(), Some("Source\\: %(title)s (%(webpage_url)s):%(meta_comment)s"));

    let mut guess = spotify_args(json!({}), "", true);
    note_the_source(&mut guess, 0.9);
    assert_eq!(tags(&guess).last().copied(), Some("Best guess, not a confirmed match. Source\\: %(title)s (%(webpage_url)s):%(meta_comment)s"));

    for args in [&confirmed, &guess] {
        let end_of_options = args.iter().position(|a| a == "--").unwrap();
        assert!(args.iter().rposition(|a| a == "--parse-metadata").unwrap() < end_of_options);
        assert_eq!(args.last().unwrap(), "https://open.spotify.com/track/abc");
    }
}

/// The finished file's real length, as ffmpeg prints it when asked about a file.
#[test]
fn the_length_ffmpeg_reports_for_a_file_is_read() {
    use ahg_engine::downloader::bridge::media_seconds;
    let report = "Input #0, mp3, from 'C:\\Music\\a.mp3':\n  Metadata:\n    title           : Duration: 99:99:99.99 is not it\n  Duration: 00:04:17.83, start: 0.025057, bitrate: 137 kb/s\n  Stream #0:0: Audio: mp3";
    assert_eq!(media_seconds(report), Some(257.83));
    assert_eq!(media_seconds("  Duration: 01:02:03.50, start: 0"), Some(3723.5));
    assert_eq!(media_seconds("  Duration: N/A, start: 0"), None);
    assert_eq!(media_seconds("no such file"), None);
    assert_eq!(media_seconds(""), None);
}
