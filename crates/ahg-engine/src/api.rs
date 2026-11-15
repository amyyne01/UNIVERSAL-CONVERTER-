//! Turning a download request into a task. Everything in the request is
//! checked before the engine acts on it: text is length-bounded, choices must
//! be known ones, and flags are coerced. The tier is enforced here, by
//! clamping a request down to what the plan allows, not by refusing it.
//!
//! Three things never come from the request: the output folder, the extras
//! (proxy, cookies, naming) and the cookies file. They come from the settings.
use serde_json::{Map, Value, json};

use crate::downloader::bridge::{GUESS_CEILING, song_url};
use crate::queue::{TaskInit, create_task};
use crate::text::js_len;
use crate::tier::{BASIC_LIMITS, Plan, clamp_audio_quality, clamp_extras, clamp_format, clamp_video_quality};
use crate::types::{AUDIO_ONLY_SOURCES, DownloadTask, PartialExtras, SourcePlatform, Track, VideoQuality};
use crate::validate::{media_url_arg, playlist_items_arg};

const MAX_URL: usize = 2048;
/// Cap on a scraped artist list, so a hostile track cannot balloon it.
const MAX_ARTISTS: usize = 64;
const BAD_TEXT: &str = "Invalid input: expected a string within length limits";

/// JavaScript truthiness of a JSON value.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// Required text of at most `max` UTF-16 units.
fn text(v: Option<&Value>, max: usize) -> Result<String, String> {
    match v.and_then(Value::as_str) {
        Some(s) if js_len(s) <= max => Ok(s.to_owned()),
        _ => Err(BAD_TEXT.into()),
    }
}

/// Optional text: absent is fine, anything present must be text within `max`.
fn opt_text(v: Option<&Value>, max: usize) -> Result<Option<String>, String> {
    v.map(|v| text(Some(v), max)).transpose()
}

fn opt_bool(v: Option<&Value>) -> Option<bool> {
    v.map(truthy)
}

/// Optional choice: absent is fine, anything present must be one of the names `T` knows.
fn opt_choice<T: serde::de::DeserializeOwned>(v: Option<&Value>, field: &str) -> Result<Option<T>, String> {
    match v {
        None => Ok(None),
        Some(v @ Value::String(_)) => serde_json::from_value(v.clone()).map(Some).map_err(|_| format!("Invalid input: {field}")),
        Some(_) => Err(format!("Invalid input: {field}")),
    }
}

fn number(v: Option<&Value>) -> Option<f64> {
    v.filter(|v| v.is_number()).and_then(Value::as_f64)
}

/// The scraped Spotify track drives the search query and the file tags, so it
/// is rebuilt from bounded fields; nothing else on it is kept.
fn validate_track(v: &Value) -> Result<Track, String> {
    let empty = Map::new();
    let t = match v {
        Value::Object(t) => t,
        Value::Array(_) => &empty,
        _ => return Err("Invalid input: track".into()),
    };
    Ok(Track {
        id: opt_text(t.get("id"), 256)?.unwrap_or_default(),
        name: text(t.get("name"), 512)?,
        artist: text(t.get("artist"), 512)?,
        artists: match t.get("artists") {
            Some(Value::Array(a)) => a.iter().take(MAX_ARTISTS).map(|x| text(Some(x), 512)).collect::<Result<_, _>>()?,
            _ => Vec::new(),
        },
        album: opt_text(t.get("album"), 512)?.unwrap_or_default(),
        album_year: opt_text(t.get("albumYear"), 16)?,
        track_number: number(t.get("trackNumber")).unwrap_or(0.0),
        duration_ms: number(t.get("durationMs")).unwrap_or(0.0),
        thumbnail_url: opt_text(t.get("thumbnailUrl"), MAX_URL)?.unwrap_or_default(),
        spotify_url: opt_text(t.get("spotifyUrl"), MAX_URL)?,
        // D34: kept only when it is a plain yes or no.
        explicit: t.get("explicit").and_then(Value::as_bool),
    })
}

/// The extras a task runs with, read from the settings. A setting holding a
/// value this version does not know (a hand-edited container name, say) is
/// left out, so that field takes its default.
fn extras_from_settings(config: &Map<String, Value>, cookies_file: Option<&str>) -> PartialExtras {
    const KEYS: [&str; 16] = [
        "subtitleMode", "subtitleLangs", "subtitleAuto", "embedChapters", "splitChapters", "sponsorBlock",
        "sponsorBlockCategories", "writeThumbnail", "writeInfoJson", "writeDescription", "videoContainer", "videoCodec",
        "cookieBrowser", "outputTemplate", "rateLimit", "proxy",
    ];
    let mut known = Map::new();
    for key in KEYS {
        if let Some(value) = config.get(key) {
            let one: Map<String, Value> = [(key.to_owned(), value.clone())].into_iter().collect();
            if serde_json::from_value::<PartialExtras>(Value::Object(one)).is_ok() {
                known.insert(key.to_owned(), value.clone());
            }
        }
    }
    // The imported cookies file sits at a fixed place and is never named by a request.
    known.insert("cookiesFile".into(), json!(cookies_file.unwrap_or("")));
    serde_json::from_value(Value::Object(known)).unwrap_or_default()
}

/// Build the task for a download request, or say why the request is refused.
///
/// `config` is the settings object, `cookies_file` the imported cookies.txt if
/// one exists, `id` a fresh unique task id and `now` the time in milliseconds.
pub fn build_task(
    request: &Value,
    config: &Map<String, Value>,
    plan: Plan,
    cookies_file: Option<&str>,
    id: String,
    now: u64,
) -> Result<DownloadTask, String> {
    let empty = Map::new();
    let req = match request {
        Value::Object(o) => o,
        Value::Array(_) => &empty,
        _ => return Err("Invalid input: expected a task object".into()),
    };
    let setting = |key: &str| config.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    let flag = |key: &str| config.get(key).is_some_and(truthy);
    let basic = plan == Plan::Basic;

    let source: Option<SourcePlatform> = opt_choice(req.get("source"), "source")?;
    // D1: a request with no audio/video choice (the clipboard offer, the hotkey,
    // an ahg:// link) takes the platform's normal one, as a link pasted into its
    // tab does: music sources are audio, the rest MP4 video. A stated choice stands.
    let by_platform = source.filter(|_| !req.contains_key("isAudioOnly"));
    let is_audio_only = match by_platform {
        Some(source) => Some(AUDIO_ONLY_SOURCES.contains(&source)),
        None => opt_bool(req.get("isAudioOnly")),
    };
    let format = match (by_platform, is_audio_only) {
        (Some(_), Some(false)) => "mp4".to_owned(),
        _ => opt_text(req.get("format"), 16)?.unwrap_or_else(|| setting("defaultFormat")),
    };
    let quality = opt_text(req.get("quality"), 16)?.unwrap_or_else(|| setting("defaultQuality"));
    let video_quality: VideoQuality = match opt_choice(req.get("videoQuality"), "videoQuality")? {
        Some(q) => q,
        // A hand-edited setting this version does not know means "no ceiling".
        None => config.get("defaultVideoQuality").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or(VideoQuality::Best),
    };
    let is_playlist = opt_bool(req.get("isPlaylist"));

    let url = match req.get("url") {
        None | Some(Value::Null) => media_url_arg("")?,
        Some(Value::String(url)) => media_url_arg(url)?,
        Some(_) => return Err(BAD_TEXT.into()),
    }
    .to_owned();
    let playlist_items = match req.get("playlistItems") {
        Some(items) if !basic => Some(
            playlist_items_arg(items.as_str().ok_or("Invalid input: playlistItems")?)?.to_owned(),
        ),
        _ => None,
    };
    let playlist_name = opt_text(req.get("playlistName"), 512)?;
    let title = opt_text(req.get("title"), 512)?;
    let thumbnail_url = opt_text(req.get("thumbnailUrl"), MAX_URL)?;
    let uploader = opt_text(req.get("uploader"), 512)?;
    let extras = clamp_extras(Some(&extras_from_settings(config, cookies_file)), plan);
    let track = req.get("track").map(validate_track).transpose()?;

    let init = TaskInit {
        url: Some(url),
        source,
        // Always the folder chosen in Settings, never one named by the request.
        output_dir: Some(setting("outputDir")),
        format: Some(clamp_format(&format, plan).to_owned()),
        quality: Some(clamp_audio_quality(&quality, plan).to_owned()),
        video_quality: Some(clamp_video_quality(video_quality, plan)),
        is_audio_only,
        is_playlist,
        playlist_name,
        embed_thumbnail: Some(opt_bool(req.get("embedThumbnail")).unwrap_or_else(|| flag("embedThumbnail"))),
        embed_metadata: Some(opt_bool(req.get("embedMetadata")).unwrap_or_else(|| flag("embedMetadata"))),
        skip_existing: Some(opt_bool(req.get("skipExisting")).unwrap_or_else(|| flag("skipExisting"))),
        title,
        thumbnail_url,
        duration: number(req.get("duration")),
        uploader,
        track,
        // The free tier's cap on a collection.
        playlist_limit: (basic && is_playlist == Some(true)).then_some(f64::from(BASIC_LIMITS.max_collection_tracks)),
        playlist_items,
        extras: serde_json::to_value(extras).ok().and_then(|v| serde_json::from_value(v).ok()),
        ..TaskInit::default()
    };
    let mut task = create_task(&init, id, now);
    // D34: the playlist preview already matched this track, so the request
    // names the video and the download does not search again.
    if source == Some(SourcePlatform::Spotify)
        && let Some(matched) = req.get("matchedUrl").filter(|v| !v.is_null())
    {
        let url = matched.as_str().and_then(song_url).ok_or("Invalid input: matchedUrl")?;
        task.matched_url = Some(url.to_owned());
        task.match_confidence = Some(number(req.get("matchConfidence")).filter(|c| (0.0..=1.0).contains(c)).unwrap_or(GUESS_CEILING));
    }
    Ok(task)
}
