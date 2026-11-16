//! Every user-visible string of the step between a pasted link or typed search
//! and a running download: a single video or track looked up, a playlist or
//! album, a short-form post, a platform search, the clipboard offer and the
//! premium locks met there. The strings are the Electron app 1.6.1's, word for
//! word (curly apostrophes, middle dots, ellipses and dashes included). Do not
//! reword them.
//!
//! `docs/design-gate/CONTENT-FOUND.md` says where each string is used, when it
//! shows and what it does; names here follow the order of that file. Several
//! strings still name a "tab" or the old structure (the platform tabs hint and
//! empty lines, the Spotify hand-off): the last section of that file ("Not
//! sure") lists them for the owner to reword. A row is (label, description) or (cause, remedy) where
//! it says so. Variable parts are `pub fn`s that return a `String`.

// ── 1. The look-up field of a platform tab (the field's own words; Home's are in `copy.rs`)
pub const LOOKUP_BUTTON_SEARCH: &str = "Search";
pub const LOOKUP_BUTTON_LOOK_UP: &str = "Look up";
pub const PASTE_FROM_CLIPBOARD: &str = "Paste from clipboard";
pub const IDLE_BATCH_HINT: &str = "Then Enter to look up. Several links, or a .txt/.csv, queue as a batch.";

/// `label` is the tab's name: `YouTube`, `Spotify`, `SoundCloud`.
pub fn lookup_placeholder_search(label: &str) -> String {
    format!("Paste a {label} link or search")
}

/// The Reels & Shorts tab has no search wording: `Paste a Reels & Shorts link`.
pub fn lookup_placeholder_link(label: &str) -> String {
    format!("Paste a {label} link")
}

// ── 2. While the look-up runs
pub const NOTE_READING: &str = "Reading the link…";
/// Added after the note once the wait passes 4 s (it begins with a space).
pub const WAIT_STILL_WORKING: &str = " Still working…";
/// Replaces the line above once the wait passes 15 s (it begins with a space).
pub const WAIT_STILL_GOING: &str = " Still going. Slow sites can take a minute.";

/// `label` is the tab's name.
pub fn note_searching(label: &str) -> String {
    format!("Searching {label}…")
}

/// A pasted list or a dropped file: `Adding 1 link…` / `Adding 3 links…`.
pub fn note_adding(count: usize) -> String {
    format!("Adding {count} link{}…", if count == 1 { "" } else { "s" })
}

// ── 3. Look-up failures and empty answers (the red panel and the dashed panel)
pub const ERROR_TRY_AGAIN: &str = "Try again";
pub const ERROR_BATCH_NONE: &str = "No downloadable links found. Check what you pasted and try again.";
pub const ERROR_DROP_WRONG_TYPE: &str = "Drop a .txt or .csv file of links.";
pub const ERROR_DROP_TOO_LARGE: &str = "That file is too large. Keep link lists under 2MB.";
pub const ERROR_DROP_NO_LINKS: &str = "That file has no links. Check it and drop it again.";
pub const ERROR_DROP_UNREADABLE: &str = "Couldn’t read that file. Check it and drop it again.";
/// Straight apostrophe, as in the source.
pub const ERROR_LINK_UNREADABLE: &str = "That link couldn't be read. Check it and try again.";
pub const ERROR_SPOTIFY_COLLECTION: &str = "Couldn’t read that Spotify collection. Check the link and try again.";
pub const ERROR_SPOTIFY_TRACK: &str = "Couldn’t read that Spotify track. Check the link and try again.";
pub const ERROR_START_FAILED: &str = "Couldn’t start the download. Try again.";
pub const ERROR_SHORT_NO_PREVIEW: &str = "That post couldn't be previewed. It may be private, removed, or need a sign-in.";
pub const EMPTY_COLLECTION: &str = "That collection is empty.";

pub fn empty_no_results(query: &str) -> String {
    format!("No results for “{query}”.")
}

pub fn empty_no_spotify_results(query: &str) -> String {
    format!("No Spotify results for “{query}”.")
}

// The second line of the "nothing found" panel, one per tab.
/// Names the tab's source; the owner may reword.
pub const EMPTY_HINT_YOUTUBE: &str = "Check the spelling, or paste the link straight from YouTube.";
pub const EMPTY_HINT_SPOTIFY: &str = "Try the artist and title together, or paste the Spotify link.";
pub const EMPTY_HINT_SOUNDCLOUD: &str = "Try a shorter query, or paste the SoundCloud link.";
pub const EMPTY_HINT_SHORT: &str = "That link had nothing to download.";

// Reasons the engine hands back for a failed look-up (shown in the red panel when
// the text is a sentence of 3 to 199 characters; otherwise ERROR_UNKNOWN below).
pub const ENGINE_TIMEOUT: &str = "Timed out reading that link — the site did not answer in time.";
pub const ENGINE_EMPTY_OUTPUT: &str = "That link could not be read.";
pub const ENGINE_REFUSED: &str = "The site refused to serve this file — try again later, or import cookies in Settings.";
pub const ENGINE_AUTH: &str = "That post needs a sign-in — it may be private, age-restricted or members-only.";
pub const ENGINE_GEO: &str = "That post is not available in your region.";
pub const ENGINE_NO_ANSWER: &str = "The site did not answer — check your connection and try again.";
pub const ENGINE_NOT_PREMIERED: &str = "That video has not premiered yet — try again once it is live.";
pub const ENGINE_REMOVED: &str = "Nothing was found at that link — it may have been removed, or the address is wrong.";
pub const ENGINE_OTHER: &str = "That post could not be read — it may be private, removed, or need a sign-in.";
pub const ENGINE_NOTHING_RETURNED: &str = "That link returned nothing to download.";

/// `last_line` is the last non-empty line of the engine's message without a leading
/// `ERROR:`, cut to 200 characters.
pub fn engine_stopped(last_line: &str) -> String {
    format!("The download stopped with an error: {last_line}")
}

// The twelve cause + remedy sentences (`describe_error`). `{thing}` in a cause
// becomes `video` for YouTube, Vimeo, Dailymotion, Rumble and Twitch, else `link`.
// The two are joined with one space.
pub const ERROR_PRIVATE: (&str, &str) =
    ("This {thing} is private.", "Ask the owner for access, or sign in with your browser in Settings, then retry.");
pub const ERROR_REMOVED: (&str, &str) = ("We couldn’t find anything at this link.", "Check the link, or paste a different one.");
pub const ERROR_AGE_RESTRICTED: (&str, &str) = ("This {thing} is age-restricted.", "Sign in with your browser in Settings, then retry.");
pub const ERROR_LOGIN_REQUIRED: (&str, &str) =
    ("This site wants you to sign in first.", "Sign in with your browser in Settings, then retry.");
pub const ERROR_GEO_BLOCKED: (&str, &str) =
    ("This {thing} isn’t available in your country.", "Paste a different link, or use a VPN you trust.");
pub const ERROR_DRM: (&str, &str) = ("This {thing} is copy-protected.", "It can’t be downloaded. Paste a different link.");
pub const ERROR_RATE_LIMITED: (&str, &str) = ("The site is asking us to slow down.", "Wait a few minutes, then retry.");
pub const ERROR_NETWORK: (&str, &str) = ("The connection dropped.", "Check your internet, then retry.");
pub const ERROR_UNSUPPORTED: (&str, &str) =
    ("This link isn’t something we can download.", "Check the link, or paste one from YouTube, Spotify, SoundCloud, Vimeo and more.");
pub const ERROR_FFMPEG_MISSING: (&str, &str) =
    ("A part of the app needed for conversion is missing.", "Download the app again, or restore the files it came with.");
pub const ERROR_DISK_FULL: (&str, &str) =
    ("There is no space left on the drive.", "Free up some space or pick another folder in Settings, then retry.");
pub const ERROR_UNKNOWN: (&str, &str) = ("Something went wrong.", "Try again in a moment.");
/// Replaces the whole sentence for a `removed` failure on a site with no tab of its own.
pub const ERROR_GENERIC_DEAD_END: &str = "We couldn’t find anything at this link. The site may not be supported, or the address is wrong. Supported sites include YouTube, Vimeo, X, TikTok, Twitch, SoundCloud, Bandcamp and Reddit.";
/// Replaces the remedy for a Spotify failure of kind unknown, removed or unsupported. Names the Spotify tab.
pub const ERROR_SPOTIFY_REMEDY: &str = "We find Spotify songs by name. Search for it in the Spotify tab, or paste a different link.";

/// Joins a cause (with `{thing}` filled in) and a remedy.
pub fn error_sentence(pair: (&str, &str), thing: &str) -> String {
    format!("{} {}", pair.0.replace("{thing}", thing), pair.1)
}

// ── 4. Format and quality (the controls above the answer, and the readout)
pub const MEDIA_VIDEO: &str = "Video";
pub const MEDIA_AUDIO: &str = "Audio";
pub const AUDIO_FORMATS: [&str; 8] = ["MP3", "AAC", "M4A", "OGG", "Opus", "FLAC", "WAV", "ALAC"];
/// The mono tag on FLAC, WAV and ALAC.
pub const TAG_LOSSLESS: &str = "lossless";
pub const AUDIO_QUALITY_LABEL: &str = "Audio quality";
pub const AUDIO_QUALITIES: [&str; 5] = ["128 kbps", "192 kbps", "256 kbps", "320 kbps", "Lossless"];
pub const VIDEO_QUALITY_LABEL: &str = "Video quality";
pub const VIDEO_QUALITIES: [&str; 6] = ["360p", "480p", "720p HD", "1080p Full HD", "2160p 4K", "Best available"];
pub const READOUT_LEAD: &str = "You get ";

/// Audio readout: `MP3 · 320 kbps`. `format` is upper-cased here.
pub fn readout_audio(format: &str, quality_label: &str) -> String {
    format!("{} · {quality_label}", format.to_uppercase())
}

/// Video readout: `MP4 · 1080p Full HD`.
pub fn readout_video(quality_label: &str) -> String {
    format!("MP4 · {quality_label}")
}

// ── 5. One video or track looked up (the single card)
pub const SINGLE_DOWNLOAD: &str = "Download";

/// Eyebrow when the detector gave no label (a Spotify track): `Spotify · Audio`.
pub fn single_eyebrow_fallback(tab_label: &str, is_audio: bool) -> String {
    format!("{tab_label} · {}", if is_audio { "Audio" } else { "Video" })
}

pub const MATCHED_LEAD: &str = "Matched by search: ";
pub const MATCHED_TRAIL: &str = ". Check the file name once it finishes.";

/// The mono part of the matched line: `Khruangbin - Texas Sun`.
pub fn matched_name(artist: &str, name: &str) -> String {
    format!("{artist} - {name}")
}

// ── 6. A search inside a platform tab (the results list)
pub const RESULTS_ARIA: &str = "Results";
pub const RESULTS_SELECT_ALL: &str = "Select all";
pub const RESULTS_CLEAR_SELECTION: &str = "Clear selection";
pub const RESULTS_KEYS: &str = "Arrows move · Space selects · Enter downloads";
pub const RESULTS_MATCHED_SUFFIX: &str = " · the audio is matched by searching artist and title";
pub const RESULTS_ROW_DOWNLOAD: &str = "Download";
pub const RESULTS_ROW_ADDED: &str = "Added";
pub const RESULTS_CLEAR: &str = "Clear";

/// `1 result` / `5 results`.
pub fn results_count(count: usize) -> String {
    format!("{count} result{}", if count == 1 { "" } else { "s" })
}

pub fn results_row_aria(title: &str, subtitle: &str) -> String {
    if subtitle.is_empty() { format!("Download {title}") } else { format!("Download {title}, {subtitle}") }
}

pub fn results_pick_aria(title: &str) -> String {
    format!("Select {title}")
}

pub fn results_selected(count: usize) -> String {
    format!("{count} selected")
}

pub fn results_download_n(count: usize) -> String {
    format!("Download {count}")
}

// Toasts raised from the results.
pub const TOAST_ADDED: &str = "Added to downloads";
pub const TOAST_VIEW_DOWNLOADS: &str = "View downloads";

pub fn toast_added_many(count: usize) -> String {
    format!("Added {count} to downloads")
}

pub fn toast_some_failed(failed: usize, total: usize) -> String {
    format!("{failed} of {total} couldn’t be added. Try those again.")
}

// ── 7. A playlist or album looked up
pub const PLAYLIST_EYEBROW_ALBUM: &str = "Spotify Album";
pub const PLAYLIST_EYEBROW_PLAYLIST: &str = "Spotify Playlist";
pub const PLAYLIST_NONE_SELECTED: &str = "none selected";
pub const PLAYLIST_SELECT_ALL: &str = "Select all";
pub const PLAYLIST_DESELECT_ALL: &str = "Deselect all";
pub const PLAYLIST_COL_NUMBER: &str = "#";
pub const PLAYLIST_COL_TITLE: &str = "Title";
pub const PLAYLIST_COL_HINT: &str = "Shift-click to pick a range";
pub const PLAYLIST_COL_ARTIST: &str = "Artist";
pub const PLAYLIST_COL_TIME: &str = "Time";
pub const PLAYLIST_FORMAT: &str = "Format";
pub const PLAYLIST_QUALITY: &str = "Quality";
pub const PLAYLIST_FOLDER: &str = "Folder";
pub const PLAYLIST_CHOOSE_FOLDER: &str = "Choose folder";
pub const PLAYLIST_FORMAT_ARIA: &str = "Playlist format";
pub const PLAYLIST_QUALITY_ARIA: &str = "Playlist quality";
pub const PLAYLIST_NOTHING_SELECTED: &str = "nothing selected";
pub const PLAYLIST_DOWNLOAD: &str = "Download";

/// Header meta line: `19 tracks · 1h 12m · by MF DOOM`. `runtime` comes from `playlist_span`.
/// The count is not pluralised in the source (`1 tracks`).
pub fn playlist_meta(track_count: usize, runtime: &str, owner: &str) -> String {
    format!("{track_count} tracks · {runtime} · by {owner}")
}

/// Milliseconds as `1h 42m` or `42m`.
pub fn playlist_span(ms: f64) -> String {
    let min = (ms / 60000.0).round() as u64;
    if min >= 60 { format!("{}h {}m", min / 60, min % 60) } else { format!("{min}m") }
}

/// Live selection counter: `3` then `/10` in a smaller size.
pub fn playlist_selected_total(total: usize) -> String {
    format!("/{total}")
}

/// Under the counter once something is picked: `selected · 12m`.
pub fn playlist_selected_span(span: &str) -> String {
    format!("selected · {span}")
}

pub fn playlist_row_aria(name: &str, artist: &str, duration: &str, capped: bool) -> String {
    let tail = if capped { ", past the Basic collection limit, will be skipped" } else { "" };
    format!("{name}, {artist}, {duration}{tail}")
}

/// Action bar readout: `8 × MP3 · 320 kbps · 31m`.
pub fn playlist_readout(count: usize, format: &str, quality_label: &str, span: &str) -> String {
    format!("{count} × {} · {quality_label} · {span}", format.to_uppercase())
}

/// The button: `Download` (nothing picked), `Download 20 of 45` (capped), `Download 3 tracks`.
pub fn playlist_download_label(selected: usize, queued: usize) -> String {
    if selected == 0 {
        "Download".to_string()
    } else if selected > queued {
        format!("Download {queued} of {selected}")
    } else {
        format!("Download {queued} track{}", if queued == 1 { "" } else { "s" })
    }
}

pub fn playlist_enqueue_error(failed: usize, total: usize) -> String {
    format!("{failed} of {total} track{} couldn’t be added.", if total == 1 { "" } else { "s" })
}

// ── 8. Basic-plan limits and premium locks met in this step
pub const BASIC_AUDIO: &str = "Basic: MP3, AAC, M4A, OGG and Opus, up to 320 kbps";
pub const BASIC_VIDEO: &str = "Basic: video up to 1080p";
pub const BASIC_COLLECTION: &str = "Basic: the first 20 tracks of any playlist";
pub const LIMIT_COLLECTION: &str = "Basic limit reached: 20 tracks per playlist. Premium removes the limit.";
pub const LIMIT_BATCH: &str = "Basic limit reached: 5 links at a time. Premium removes the limit.";
pub const LIMIT_SKIPPED_LEAD: &str = "The last ";
pub const LIMIT_SKIPPED_TRAIL: &str = " you picked won’t be downloaded.";
pub const SEE_PLANS: &str = "See plans";
pub const LOCK_PICK_PLAYLIST: &str = "Choosing videos from a playlist is part of Premium. Basic downloads the playlist from the top.";
pub const LOCK_LOSSLESS_QUALITY: &str = "Lossless audio is a Premium feature.";
pub const LOCK_SELECT_TITLE: &str = "Options marked Premium need the Premium plan. Select one to see what it adds.";
pub const LOCK_OPTION_TITLE: &str = "Needs Premium";
pub const LOCK_OPTION_SUFFIX: &str = " · Premium";

/// A locked format, by its label: `FLAC is a Premium feature.`
pub fn lock_format(label: &str) -> String {
    format!("{label} is a Premium feature.")
}

/// A locked video quality, by the raw value (`2160p`, `best`): `2160p video is a Premium feature.`
pub fn lock_video_quality(value: &str) -> String {
    format!("{value} video is a Premium feature.")
}

pub fn lock_chip_aria(label: &str) -> String {
    format!("{label}, Premium")
}

pub fn lock_chip_title(label: &str) -> String {
    format!("{label} needs Premium. Select it to see what the plan adds.")
}

// ── 9. A short-form post (Reels, TikTok, Facebook)
pub const SHORT_DOWNLOAD: &str = "Download";
pub const SHORT_CAPABILITY: &str = "Video · MP4 · No watermark";
pub const SHORT_HINT: &str = "Paste an Instagram Reel, TikTok or Facebook link. The video arrives without the watermark.";

/// Facts line parts: `{width}x{height}`.
pub fn short_size(width: u32, height: u32) -> String {
    format!("{width}x{height}")
}

pub fn short_views(formatted_count: &str) -> String {
    format!("{formatted_count} views")
}

pub fn short_likes(formatted_count: &str) -> String {
    format!("{formatted_count} likes")
}

// ── 10. What each tab says about itself (the page header and the empty state)
pub const YOUTUBE_CAPABILITY: &str = "Video + Audio · MP4/MP3 · Playlists";
pub const YOUTUBE_HINT: &str = "Paste a video or playlist link, or type what you want to search YouTube";
pub const SPOTIFY_CAPABILITY: &str = "Audio · MP3/FLAC · Playlists + Albums";
pub const SPOTIFY_HINT: &str =
    "Paste a track, album or playlist link, or search by song and artist. Tracks are matched to the best available audio source.";
pub const SOUNDCLOUD_CAPABILITY: &str = "Audio · MP3/FLAC · Artist sets";
pub const SOUNDCLOUD_HINT: &str = "Paste a track or set link, or search SoundCloud by artist, title or set";

// ── 11. The clipboard offer
pub const CLIPBOARD_DOWNLOAD: &str = "Download";
pub const CLIPBOARD_DISMISS: &str = "Dismiss";

/// `YouTube video copied`, `Spotify playlist copied`, `Direct media copied`.
pub fn clipboard_title(label: &str) -> String {
    format!("{label} copied")
}

/// The Settings row that switches the offer on (also in `copy/settings.rs` if present there).
pub const CLIPBOARD_SETTING: (&str, &str) =
    ("Clipboard watcher", "Offer a one-click download whenever you copy a link, even outside the app");
