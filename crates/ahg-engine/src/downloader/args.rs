//! The yt-dlp command line for a task: `Downloader.buildYtDlpArgs` in
//! electron/downloader.ts, down to the way Node's `path.win32.join` writes
//! the output path.
use crate::text::js_trim;
use crate::types::{
    CookieBrowser, LOSSLESS_FORMATS, SponsorBlockMode, SubtitleMode, VideoCodec, VideoContainer, VideoQuality,
    DownloadTask, merge_extras,
};

/// Categories SponsorBlock accepts (`SPONSORBLOCK_CATEGORIES`).
const SPONSORBLOCK_CATEGORIES: [&str; 7] =
    ["sponsor", "intro", "outro", "selfpromo", "interaction", "preview", "music_offtopic"];
/// Targets yt-dlp can embed cover art into.
const THUMB_TARGETS: [&str; 8] = ["mp3", "m4a", "alac", "ogg", "opus", "flac", "mp4", "mkv"];
const FALLBACK_PLAYER_CLIENTS: &str = "android,mweb,tv_simply";
const PROGRESS_TEMPLATE: &str = "download:[PROGRESS]%(progress.downloaded_bytes)s|%(progress.total_bytes)s\
|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s";
const FILEPATH_MARKER: &str = "[FILEPATH]";

/// The full argument list; the target URL is the last element.
pub fn build_ytdlp_args(task: &DownloadTask, ffmpeg_path: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = [
        "--no-warnings", "--newline", "--socket-timeout", "30", "--continue", "--progress-template", PROGRESS_TEMPLATE,
        "--retries", "10", "--concurrent-fragments", "4", "--print",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.push(format!("after_move:{FILEPATH_MARKER}%(filepath)s"));
    args.push("--no-quiet".into());

    // An empty path is as good as none, as it was for `if (ffmpegPath)`.
    if let Some(p) = ffmpeg_path.filter(|p| !p.is_empty()) {
        push(&mut args, &["--ffmpeg-location", p]);
    }
    if task.fallback_profile == Some(true) {
        push(&mut args, &["--extractor-args", &format!("youtube:player_client={FALLBACK_PLAYER_CLIENTS}")]);
    }

    let x = merge_extras(task.extras.as_ref());
    let container = match x.video_container {
        VideoContainer::Mp4 => "mp4",
        VideoContainer::Mkv => "mkv",
        VideoContainer::Webm => "webm",
    };

    if !x.rate_limit.is_empty() {
        push(&mut args, &["--limit-rate", &x.rate_limit]);
    }
    if !x.proxy.is_empty() {
        push(&mut args, &["--proxy", &x.proxy]);
    }
    if !x.cookies_file.is_empty() && win32_is_absolute(&x.cookies_file) {
        push(&mut args, &["--cookies", &x.cookies_file]);
    } else if let Some(b) = cookie_browser(x.cookie_browser) {
        push(&mut args, &["--cookies-from-browser", b]);
    }

    if task.is_audio_only {
        let codec = task.format.as_str();
        push(&mut args, &["-f", "bestaudio/best", "--extract-audio", "--audio-format", codec]);
        if !LOSSLESS_FORMATS.contains(&codec) && !task.quality.is_empty() && task.quality != "lossless" {
            push(&mut args, &["--audio-quality", &format!("{}K", task.quality)]);
        }
    } else {
        push(&mut args, &["-f", &video_selector(task.video_quality, x.video_container, x.video_codec)]);
        push(&mut args, &["--merge-output-format", container]);
    }

    if task.embed_metadata {
        args.push("--embed-metadata".into());
    }
    let thumb_target = if task.is_audio_only { task.format.as_str() } else { container };
    if task.embed_thumbnail && THUMB_TARGETS.contains(&thumb_target) {
        args.push("--embed-thumbnail".into());
    }
    if task.skip_existing {
        args.push("--no-overwrites".into());
    }

    if x.subtitle_mode != SubtitleMode::Off {
        let langs = if x.subtitle_langs.is_empty() { "en" } else { &x.subtitle_langs };
        push(&mut args, &["--sub-langs", langs]);
        if x.subtitle_auto {
            args.push("--write-auto-subs".into());
        }
        if matches!(x.subtitle_mode, SubtitleMode::Embed | SubtitleMode::Both) {
            args.push("--embed-subs".into());
        }
        if matches!(x.subtitle_mode, SubtitleMode::File | SubtitleMode::Both) {
            push(&mut args, &["--write-subs", "--convert-subs", "srt"]);
        }
    }

    if x.embed_chapters {
        args.push("--embed-chapters".into());
    }
    if x.split_chapters {
        args.push("--split-chapters".into());
    }

    if x.sponsor_block != SponsorBlockMode::Off {
        let cats: Vec<&str> = x
            .sponsor_block_categories
            .iter()
            .map(String::as_str)
            .filter(|c| SPONSORBLOCK_CATEGORIES.contains(c))
            .collect();
        if !cats.is_empty() {
            let flag = if x.sponsor_block == SponsorBlockMode::Remove { "--sponsorblock-remove" } else { "--sponsorblock-mark" };
            push(&mut args, &[flag, &cats.join(",")]);
        }
    }

    if x.write_thumbnail {
        args.push("--write-thumbnail".into());
    }
    if x.write_info_json {
        args.push("--write-info-json".into());
    }
    if x.write_description {
        args.push("--write-description".into());
    }

    let sub = if task.is_playlist { sanitize(&task.playlist_name) } else { String::new() };
    let template = safe_template(&x.output_template);
    let tag = if task.is_audio_only {
        let f = task.format.as_str();
        // /^[a-z0-9]+$/ : `$` does not stop before a trailing newline in JavaScript.
        if f != "mp3" && !f.is_empty() && f.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
            format!(" [{f}]")
        } else {
            String::new()
        }
    } else if task.video_quality != VideoQuality::Best {
        format!(" [{}]", quality_name(task.video_quality))
    } else {
        String::new()
    };
    let name = if !template.is_empty() {
        format!("{template}.%(ext)s")
    } else {
        let stem = if task.is_playlist {
            "%(title)s".to_owned()
        } else {
            let t = sanitize(&task.title);
            if t.is_empty() { "%(title)s".to_owned() } else { t }
        };
        format!("{stem}{tag}.%(ext)s")
    };
    push(&mut args, &["-o", &win32_join(&[&task.output_dir, &sub, &name])]);
    if x.split_chapters {
        let chapter = win32_join(&[&task.output_dir, &sub, "%(title)s - %(section_number)03d %(section_title)s.%(ext)s"]);
        push(&mut args, &["-o", &format!("chapter:{chapter}")]);
    }
    if task.is_playlist {
        push(&mut args, &["--download-archive", &win32_join(&[&task.output_dir, &sub, ".download-archive.txt"])]);
        if let Some(limit) = task.playlist_limit.filter(|l| *l > 0.0) {
            push(&mut args, &["--playlist-end", &js_number(limit.floor())]);
        }
        if let Some(items) = task.playlist_items.as_deref().filter(|i| !i.is_empty()) {
            push(&mut args, &["--playlist-items", items]);
        }
        push(&mut args, &["--sleep-requests", "1", "--sleep-interval", "1"]);
    } else {
        args.push("--no-playlist".into());
    }

    push(&mut args, &["--", &task.url]);
    args
}

fn push(args: &mut Vec<String>, more: &[&str]) {
    args.extend(more.iter().map(|s| s.to_string()));
}

fn cookie_browser(b: CookieBrowser) -> Option<&'static str> {
    Some(match b {
        CookieBrowser::None => return None,
        CookieBrowser::Chrome => "chrome",
        CookieBrowser::Edge => "edge",
        CookieBrowser::Firefox => "firefox",
        CookieBrowser::Brave => "brave",
        CookieBrowser::Chromium => "chromium",
        CookieBrowser::Opera => "opera",
        CookieBrowser::Vivaldi => "vivaldi",
    })
}

fn quality_name(q: VideoQuality) -> &'static str {
    match q {
        VideoQuality::P360 => "360p",
        VideoQuality::P480 => "480p",
        VideoQuality::P720 => "720p",
        VideoQuality::P1080 => "1080p",
        VideoQuality::P2160 => "2160p",
        VideoQuality::Best => "best",
    }
}

/// A preference with fallbacks: the codec in the container at or below the
/// height, dropping one constraint at a time.
fn video_selector(quality: VideoQuality, container: VideoContainer, codec: VideoCodec) -> String {
    let h = match quality {
        VideoQuality::P360 => Some(360),
        VideoQuality::P480 => Some(480),
        VideoQuality::P720 => Some(720),
        VideoQuality::P1080 => Some(1080),
        VideoQuality::P2160 => Some(2160),
        VideoQuality::Best => None,
    };
    let height = h.map(|h| format!("[height<={h}]")).unwrap_or_default();
    let vcodec = match codec {
        VideoCodec::Any => "",
        VideoCodec::H264 => "[vcodec^=avc1]",
        VideoCodec::Vp9 => "[vcodec^=vp9]",
        VideoCodec::Av1 => "[vcodec^=av01]",
    };
    let mp4 = container == VideoContainer::Mp4;
    let ext = if mp4 { "[ext=mp4]" } else { "" };
    let aext = if mp4 { "[ext=m4a]" } else { "" };
    [
        format!("bestvideo{height}{vcodec}{ext}+bestaudio{aext}"),
        format!("bestvideo{height}{vcodec}+bestaudio"),
        format!("bestvideo{height}{ext}+bestaudio{aext}"),
        format!("bestvideo{height}+bestaudio"),
        if h.is_some() { format!("best{height}") } else { "best".to_owned() },
        "best".to_owned(),
    ]
    .join("/")
}

/// A user template, or "" when it could leave the output folder.
fn safe_template(template: &str) -> String {
    let t = js_trim(template).replace('\\', "/");
    let mut chars = t.chars();
    let drive = matches!((chars.next(), chars.next()), (Some(c), Some(':')) if c.is_ascii_alphabetic());
    if t.starts_with('/') || drive || t.split('/').any(|seg| seg == "..") {
        return String::new();
    }
    t
}

/// A file or folder name with path and control characters removed.
fn sanitize(s: &str) -> String {
    let stripped: String = s.chars().filter(|c| !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0'..='\u{1f}')).collect();
    let t = js_trim(&stripped);
    if !t.is_empty() && t.bytes().all(|b| b == b'.') {
        return String::new();
    }
    if is_reserved_device(t) {
        return format!("{t}_");
    }
    t.to_owned()
}

/// /^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/i
fn is_reserved_device(s: &str) -> bool {
    let u = s.to_ascii_uppercase();
    matches!(u.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (u.len() == 4 && (u.starts_with("COM") || u.starts_with("LPT")) && matches!(u.as_bytes()[3], b'1'..=b'9'))
}

/// `String(n)` for a whole number.
fn js_number(n: f64) -> String {
    if n.is_infinite() {
        "Infinity".to_owned()
    } else if n.abs() < 1e21 {
        format!("{n}")
    } else {
        format!("{n:e}").replacen('e', "e+", 1)
    }
}

// ── Node's path.win32, on UTF-16 text (version 24) ───────────────────────────

const BACKSLASH: u16 = b'\\' as u16;
const SLASH: u16 = b'/' as u16;
const COLON: u16 = b':' as u16;
const DOT: u16 = b'.' as u16;

fn is_sep(c: u16) -> bool {
    c == BACKSLASH || c == SLASH
}

fn is_device_root(c: u16) -> bool {
    u8::try_from(c).is_ok_and(|b| b.is_ascii_alphabetic())
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn string(v: &[u16]) -> String {
    String::from_utf16_lossy(v)
}

/// `s.slice(start, end)` with JavaScript's handling of negative bounds.
fn slice(s: &[u16], start: isize, end: isize) -> &[u16] {
    let len = s.len() as isize;
    let at = |i: isize| if i < 0 { (len + i).max(0) } else { i.min(len) } as usize;
    let (a, b) = (at(start), at(end));
    if a >= b { &[] } else { &s[a..b] }
}

fn index_of(s: &[u16], c: u16, from: usize) -> isize {
    s.iter().skip(from).position(|&x| x == c).map_or(-1, |p| (p + from) as isize)
}

/// Node's `isWindowsReservedName(path, colonIndex)`: whether the text before the
/// index is CON, PRN, AUX, NUL, COM1-9 or LPT1-9 (also with the superscript
/// digits). An index of -1, no colon, drops the last character; that is
/// what Node does, so a plain `CONx` is "reserved".
fn is_win_reserved(path: &[u16], colon: isize) -> bool {
    let s = slice(path, 0, colon);
    let up: Vec<u16> = s.iter().map(|&c| if (b'a' as u16..=b'z' as u16).contains(&c) { c - 32 } else { c }).collect();
    let is = |t: &str| up == utf16(t);
    is("CON") || is("PRN") || is("AUX") || is("NUL") || (up.len() == 4 && {
        let head = string(&up[..3]);
        (head == "COM" || head == "LPT") && matches!(up[3], 0x31..=0x39 | 0xB2 | 0xB3 | 0xB9)
    })
}

/// `path.win32.isAbsolute`.
fn win32_is_absolute(p: &str) -> bool {
    let p = utf16(p);
    match p.first() {
        None => false,
        Some(&c) => is_sep(c) || (p.len() > 2 && is_device_root(c) && p[1] == COLON && is_sep(p[2])),
    }
}

/// `path.win32.join`.
fn win32_join(parts: &[&str]) -> String {
    let parts: Vec<Vec<u16>> = parts.iter().filter(|p| !p.is_empty()).map(|p| utf16(p)).collect();
    if parts.is_empty() {
        return ".".to_owned();
    }
    let first = &parts[0];
    let mut joined: Vec<u16> = parts.join(&BACKSLASH);

    // A leading run of slashes would read as a UNC root, unless the first part is already one.
    let mut needs_replace = true;
    let mut slashes = 0;
    if is_sep(first[0]) {
        slashes = 1;
        if first.len() > 1 && is_sep(first[1]) {
            slashes = 2;
            if first.len() > 2 {
                if is_sep(first[2]) {
                    slashes = 3;
                } else {
                    needs_replace = false;
                }
            }
        }
    }
    if needs_replace {
        while slashes < joined.len() && is_sep(joined[slashes]) {
            slashes += 1;
        }
        if slashes >= 2 {
            joined = [&[BACKSLASH][..], &joined[slashes..]].concat();
        }
    }

    // A part like `CON:x` is left alone apart from its slashes.
    let reserved_part = joined.split(|&c| c == BACKSLASH).filter(|p| !p.is_empty()).any(|p| {
        let colon = index_of(p, COLON, 0);
        colon != -1 && is_win_reserved(p, colon)
    });
    if reserved_part {
        return string(&joined.iter().map(|&c| if c == SLASH { BACKSLASH } else { c }).collect::<Vec<_>>());
    }
    string(&normalize(&joined))
}

/// `path.win32.normalize`.
fn normalize(path: &[u16]) -> Vec<u16> {
    let len = path.len();
    if len == 0 {
        return vec![DOT];
    }
    let code = path[0];
    if len == 1 {
        return if code == SLASH { vec![BACKSLASH] } else { path.to_vec() };
    }
    let mut root_end = 0;
    let mut device: Option<Vec<u16>> = None;
    let mut absolute = false;

    if is_sep(code) {
        absolute = true;
        if is_sep(path[1]) {
            let mut j = 2;
            let mut last = j;
            while j < len && !is_sep(path[j]) {
                j += 1;
            }
            if j < len && j != last {
                let first_part = &path[last..j];
                last = j;
                while j < len && is_sep(path[j]) {
                    j += 1;
                }
                if j < len && j != last {
                    last = j;
                    while j < len && !is_sep(path[j]) {
                        j += 1;
                    }
                    if j == len || j != last {
                        if first_part == [DOT] || first_part == [b'?' as u16] {
                            // A device root such as \\.\PHYSICALDRIVE0.
                            device = Some([&[BACKSLASH, BACKSLASH][..], first_part].concat());
                            root_end = 4;
                            let possible = slice(path, 4, index_of(path, COLON, 0) + 1);
                            if is_win_reserved(possible, possible.len() as isize - 1) {
                                device = Some([&utf16("\\\\?\\")[..], possible].concat());
                                root_end = 4 + possible.len();
                            }
                        } else if j == len {
                            // A UNC root and nothing else.
                            return [&[BACKSLASH, BACKSLASH][..], first_part, &[BACKSLASH], &path[last..], &[BACKSLASH]].concat();
                        } else {
                            device = Some([&[BACKSLASH, BACKSLASH][..], first_part, &[BACKSLASH], &path[last..j]].concat());
                            root_end = j;
                        }
                    }
                }
            }
        } else {
            root_end = 1;
        }
    } else {
        let colon = index_of(path, COLON, 0);
        if colon > 0 {
            if is_device_root(code) && colon == 1 {
                device = Some(path[..2].to_vec());
                root_end = 2;
                if len > 2 && is_sep(path[2]) {
                    absolute = true;
                    root_end = 3;
                }
            } else if is_win_reserved(path, colon) {
                device = Some(path[..colon as usize + 1].to_vec());
                root_end = colon as usize + 1;
            }
        }
    }

    let mut tail = if root_end < len { normalize_string(&path[root_end..], !absolute) } else { Vec::new() };
    if tail.is_empty() && !absolute {
        tail = vec![DOT];
    }
    if !tail.is_empty() && is_sep(path[len - 1]) {
        tail.push(BACKSLASH);
    }
    let dot_slash = |tail: &[u16]| [&[DOT, BACKSLASH][..], tail].concat();
    if !absolute && device.is_none() && path.contains(&COLON) {
        // The tail must not turn into something Windows reads as a drive.
        if tail.len() >= 2 && is_device_root(tail[0]) && tail[1] == COLON {
            return dot_slash(&tail);
        }
        let mut index = index_of(path, COLON, 0);
        while index != -1 {
            let i = index as usize;
            if i == len - 1 || is_sep(path[i + 1]) {
                return dot_slash(&tail);
            }
            index = index_of(path, COLON, i + 1);
        }
    }
    if is_win_reserved(path, index_of(path, COLON, 0)) {
        return dot_slash(&[device.as_deref().unwrap_or_default(), &tail].concat());
    }
    match (device, absolute) {
        (None, false) => tail,
        (None, true) => [&[BACKSLASH][..], &tail].concat(),
        (Some(d), false) => [d, tail].concat(),
        (Some(d), true) => [d, vec![BACKSLASH], tail].concat(),
    }
}

/// Node's `normalizeString`: resolves `.` and `..` and collapses separators.
fn normalize_string(path: &[u16], allow_above_root: bool) -> Vec<u16> {
    let mut res: Vec<u16> = Vec::new();
    let mut last_segment_len = 0usize;
    let mut last_slash: isize = -1;
    let mut dots: isize = 0;
    let mut code = 0u16;
    for i in 0..=path.len() {
        if i < path.len() {
            code = path[i];
        } else if is_sep(code) {
            break;
        } else {
            code = SLASH;
        }
        if is_sep(code) {
            if last_slash == i as isize - 1 || dots == 1 {
                // Nothing to add.
            } else if dots == 2 {
                if res.len() < 2 || last_segment_len != 2 || res[res.len() - 1] != DOT || res[res.len() - 2] != DOT {
                    if res.len() > 2 {
                        match res.iter().rposition(|&c| c == BACKSLASH) {
                            None => {
                                res.clear();
                                last_segment_len = 0;
                            }
                            Some(p) => {
                                res.truncate(p);
                                let prev = res.iter().rposition(|&c| c == BACKSLASH).map_or(-1, |q| q as isize);
                                last_segment_len = (res.len() as isize - 1 - prev) as usize;
                            }
                        }
                        last_slash = i as isize;
                        dots = 0;
                        continue;
                    } else if !res.is_empty() {
                        res.clear();
                        last_segment_len = 0;
                        last_slash = i as isize;
                        dots = 0;
                        continue;
                    }
                }
                if allow_above_root {
                    if !res.is_empty() {
                        res.push(BACKSLASH);
                    }
                    res.extend([DOT, DOT]);
                    last_segment_len = 2;
                }
            } else {
                let seg = &path[(last_slash + 1) as usize..i];
                if !res.is_empty() {
                    res.push(BACKSLASH);
                }
                res.extend_from_slice(seg);
                last_segment_len = (i as isize - last_slash - 1) as usize;
            }
            last_slash = i as isize;
            dots = 0;
        } else if code == DOT && dots != -1 {
            dots += 1;
        } else {
            dots = -1;
        }
    }
    res
}
