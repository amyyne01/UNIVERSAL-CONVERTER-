//! Cover art in live mode: every picture the screen shows (a task's, the found step's, a search
//! row's) is fetched once by URL through the core's strict cover client (`Core::fetch_cover`:
//! https only, the host resolved and pinned, 5 MiB at most) on a worker thread, its size read
//! from its header before it is decoded, and kept here. A preset draws a cover through [`get`]
//! (or [`State::cover`] for a task) over its own placeholder, which stays while the picture
//! loads and when it fails; the picture fades in over it ([`art`]).
use std::collections::{HashMap, HashSet, VecDeque};

use rune::{El, Image, ImageCache, SurfaceFit, el};

use crate::gate::{Band, Found, State};

/// The key of a fetched picture laid over its placeholder.
pub const ART: &str = "cover-art";
/// Fetches running at once.
const IN_FLIGHT: usize = 3;
/// Pictures kept, and wanted at once: the found step's first, then the newest tasks'.
// ponytail: rows past the 64th keep their placeholder; a scroll-aware want lifts that.
const KEPT: usize = 32;
/// Decoded bytes kept (pixels, 4 bytes each); pictures on show are never evicted for it.
const KEPT_BYTES: u64 = 192 * 1024 * 1024;
/// The largest side taken (LESSONS 24e: the byte cap bounds the file, not its pixels). A thumbnail
/// needs far less; 64 kept at 1024 x 1024 x 4 bytes bounds even the pictures on show to 256 MB.
const MAX_SIDE: u32 = 4096;
/// And the most pixels: a 1920 x 1080 picture (YouTube's best thumbnail is 1280 x 720 or this) fits,
/// a 1024 side cap did not. 32 kept at this size bound even the pictures on show to about 265 MB.
const MAX_PIXELS: u64 = 1920 * 1080;

/// What is fetched, kept and refused, by URL.
#[derive(Default)]
pub struct Covers {
    kept: HashMap<String, Image>,
    /// `kept`'s URLs, least recently on show first.
    order: VecDeque<String>,
    /// Wanted, not asked yet.
    queue: VecDeque<String>,
    asked: HashSet<String>,
    /// Failed, refused or undecodable: not asked again this run.
    refused: HashSet<String>,
    /// On show at the last [`sync`].
    shown: HashSet<String>,
}

/// A fetched picture, once it is decoded.
pub fn get<'a>(s: &'a State, url: &str) -> Option<&'a Image> {
    s.thumbs.kept.get(url)
}

/// The URLs the screen shows a picture for: the found step's, then the tasks' (newest first).
fn on_show(s: &State) -> Vec<String> {
    let mut urls: Vec<&str> = match &s.found {
        Found::Media(m) => vec![&m.thumbnail_url],
        // No preset draws a track's own picture: only the collection's.
        Found::Playlist { playlist, .. } => vec![&playlist.thumbnail_url],
        Found::Search { bands, .. } => bands
            .iter()
            .flat_map(|b| match b {
                Band::Results(rs) => rs.iter().map(|r| r.thumbnail_url.as_str()).collect(),
                _ => Vec::new(),
            })
            .collect(),
        _ => Vec::new(),
    };
    urls.extend(s.tasks.iter().map(|t| t.thumbnail_url.as_str()));
    let mut seen = HashSet::new();
    urls.into_iter().filter(|u| !u.is_empty() && seen.insert(*u)).take(KEPT).map(str::to_owned).collect()
}

/// Brings the fetches to what is on show: those kept become the most recent, the rest are
/// wanted, and fetches start. Live mode only; called by the poll that applied new data.
pub fn sync(s: &mut State) {
    if s.live.is_none() {
        return;
    }
    let shown = on_show(s);
    let c = &mut s.thumbs;
    c.order.retain(|u| !shown.contains(u));
    c.order.extend(shown.iter().filter(|u| c.kept.contains_key(*u)).cloned());
    c.shown = shown.iter().cloned().collect();
    c.queue.clear();
    for url in &shown {
        want(s, url);
    }
    pump(s);
}

/// Asks for `url` once: nothing when it is kept, asked, queued or refused.
pub fn want(s: &mut State, url: &str) {
    let c = &mut s.thumbs;
    if url.is_empty() || c.kept.contains_key(url) || c.asked.contains(url) || c.refused.contains(url) || c.queue.iter().any(|u| u == url) {
        return;
    }
    c.queue.push_back(url.to_owned());
}

/// Starts queued fetches up to [`IN_FLIGHT`].
fn pump(s: &mut State) {
    while s.thumbs.asked.len() < IN_FLIGHT {
        let Some(url) = s.thumbs.queue.pop_front() else { return };
        s.thumbs.asked.insert(url.clone());
        crate::live::fetch_cover(s, url);
    }
}

/// A fetch answered: a decoded picture, or `None` when it failed. True when a picture arrived.
pub fn arrived(s: &mut State, url: String, image: Option<Image>) -> bool {
    let c = &mut s.thumbs;
    c.asked.remove(&url);
    let came = image.is_some();
    match image {
        Some(image) => {
            c.kept.insert(url.clone(), image);
            c.order.push_back(url);
            evict(c);
        }
        None => {
            c.refused.insert(url);
        }
    }
    pump(s);
    came
}

/// Drops the least recently shown pictures past [`KEPT`] or [`KEPT_BYTES`], never one on show.
fn evict(c: &mut Covers) {
    let bytes = |c: &Covers| c.kept.values().filter_map(Image::size).map(|[w, h]| u64::from(w) * u64::from(h) * 4).sum::<u64>();
    while c.kept.len() > KEPT || bytes(c) > KEPT_BYTES {
        let Some(at) = c.order.iter().position(|u| !c.shown.contains(u)) else { return };
        if let Some(oldest) = c.order.remove(at) {
            c.kept.remove(&oldest);
        }
    }
}

/// Decodes fetched bytes on the calling (worker) thread: `None` for a format other than PNG,
/// JPEG or WebP (a GIF or any animation is refused), a side above [`MAX_SIDE`], or a decode that fails.
pub fn decode(cache: &ImageCache, bytes: Vec<u8>) -> Option<Image> {
    let (w, h) = size(&bytes)?;
    if !fits(w, h) {
        return None;
    }
    if animated(&bytes) {
        return None;
    }
    let image = cache.load(bytes);
    image.wait().ok()?;
    Some(image)
}

/// Whether a picture of this size is taken: no empty side, none past [`MAX_SIDE`], at most [`MAX_PIXELS`].
fn fits(w: u32, h: u32) -> bool {
    w != 0 && h != 0 && w <= MAX_SIDE && h <= MAX_SIDE && u64::from(w) * u64::from(h) <= MAX_PIXELS
}

/// A GIF, an animated WebP (`ANIM`) or an APNG (`acTL`): every frame is decoded and kept, so the
/// pixel budget would be under-counted. Covers are stills; these are refused.
fn animated(b: &[u8]) -> bool {
    let has = |tag: &[u8; 4]| b.windows(4).any(|w| w == tag);
    b.starts_with(b"GIF8") || (b.starts_with(b"RIFF") && has(b"ANIM")) || (b.starts_with(b"\x89PNG") && has(b"acTL"))
}

/// Width and height from a PNG, GIF, WebP or JPEG header; `None` for anything else.
fn size(b: &[u8]) -> Option<(u32, u32)> {
    let be16 = |i: usize| Some(u32::from(u16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?)));
    let le16 = |i: usize| Some(u32::from(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?)));
    let be32 = |i: usize| Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?));
    let le24 = |i: usize| Some(u32::from(*b.get(i)?) | u32::from(*b.get(i + 1)?) << 8 | u32::from(*b.get(i + 2)?) << 16);
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some((be32(16)?, be32(20)?));
    }
    if b.starts_with(b"GIF8") {
        return Some((le16(6)?, le16(8)?));
    }
    if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        return match b.get(12..16)? {
            b"VP8 " => Some((le16(26)? & 0x3fff, le16(28)? & 0x3fff)),
            b"VP8L" => {
                let bits = le24(21)? | u32::from(*b.get(24)?) << 24;
                Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
            }
            b"VP8X" => Some((le24(24)? + 1, le24(27)? + 1)),
            _ => None,
        };
    }
    if b.starts_with(&[0xff, 0xd8]) {
        // The first frame header (SOF0..SOF15 but DHT, JPG and DAC) holds the size.
        let mut i = 2;
        while *b.get(i)? == 0xff {
            let marker = *b.get(i + 1)?;
            if (0xc0..=0xcf).contains(&marker) && ![0xc4, 0xc8, 0xcc].contains(&marker) {
                return Some((be16(i + 7)?, be16(i + 5)?));
            }
            i += 2 + be16(i + 2)? as usize;
        }
    }
    None
}

/// The picture laid over its placeholder, filling it with corners of `radius`, fading in on the
/// look's cover token (`motion::cover_in`). The placeholder under it stays the element's size,
/// so the layout never moves.
pub fn art(s: &State, image: &Image, radius: f32) -> El {
    el().key(ART)
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .radius(radius)
        .pixels_fit(image.pixels(), SurfaceFit::Fill)
        .transition(crate::motion::cover_in(s))
}

/// `placeholder` with `url`'s picture over it once that is here.
pub fn over(s: &State, placeholder: El, url: &str, radius: f32) -> El {
    match get(s, url) {
        Some(image) => placeholder.child(art(s, image, radius)),
        None => placeholder,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A `w` x `h` PNG of one colour: zlib's stored blocks, no compression.
    pub fn png(w: u32, h: u32) -> Vec<u8> {
        fn crc(data: &[u8]) -> u32 {
            let mut c = !0u32;
            for &b in data {
                c ^= u32::from(b);
                for _ in 0..8 {
                    c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
                }
            }
            !c
        }
        let line: Vec<u8> = std::iter::once(0).chain([200, 60, 40, 255].repeat(w as usize)).collect();
        let raw = line.repeat(h as usize);
        let mut z = vec![0x78, 0x01];
        let chunks: Vec<&[u8]> = raw.chunks(65535).collect();
        for (i, part) in chunks.iter().enumerate() {
            let n = part.len() as u16;
            z.push(u8::from(i + 1 == chunks.len()));
            z.extend(n.to_le_bytes());
            z.extend((!n).to_le_bytes());
            z.extend(*part);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &x in &raw {
            a = (a + u32::from(x)) % 65521;
            b = (b + a) % 65521;
        }
        z.extend((b << 16 | a).to_be_bytes());
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = w.to_be_bytes().to_vec();
        ihdr.extend(h.to_be_bytes());
        ihdr.extend([8, 6, 0, 0, 0]);
        for (kind, data) in [(&b"IHDR"[..], ihdr), (b"IDAT", z), (b"IEND", Vec::new())] {
            out.extend((data.len() as u32).to_be_bytes());
            let mut body = kind.to_vec();
            body.extend(&data);
            out.extend(&body);
            out.extend(crc(&body).to_be_bytes());
        }
        out
    }

    #[test]
    fn the_header_gives_the_size_and_unknown_or_huge_pictures_are_refused() {
        assert_eq!(size(&png(3, 2)), Some((3, 2)));
        let gif = [b"GIF89a".as_slice(), &[5, 0, 7, 0]].concat();
        assert_eq!(size(&gif), Some((5, 7)));
        // A JPEG: SOI, an APP0 of 4 bytes, then SOF0 with height 9 and width 11.
        let jpeg = [0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 11, 8, 0, 9, 0, 11, 3];
        assert_eq!(size(&jpeg), Some((11, 9)));
        let webp = [b"RIFF\0\0\0\0WEBPVP8X".as_slice(), &[0; 8], &[9, 0, 0, 4, 0, 0]].concat();
        assert_eq!(size(&webp), Some((10, 5)));
        assert_eq!(size(b"<svg xmlns='http://www.w3.org/2000/svg'/>"), None, "SVG is not taken");
        assert_eq!(size(b""), None);
        assert_eq!(size(&jpeg[..10]), None, "a cut header");
        let cache = ImageCache::new(std::sync::Arc::new(|| {}));
        assert!(decode(&cache, png(MAX_SIDE + 1, 1)).is_none(), "too wide");
        assert!(fits(1280, 720) && fits(1920, 1080) && fits(640, 640), "real thumbnails fit");
        assert!(!fits(2000, 2000) && !fits(4096, 4096) && !fits(0, 5), "a huge or empty picture does not");
        assert!(decode(&cache, gif.clone()).is_none(), "a GIF is animated");
        assert!(animated(&[b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0ANIM".as_slice()].concat()), "animated WebP");
        assert!(animated(&[png(2, 2), b"acTL".to_vec()].concat()), "APNG");
        assert!(!animated(&png(2, 2)), "a still PNG");
        assert!(decode(&cache, b"\x89PNG\r\n\x1a\nbroken".to_vec()).is_none(), "undecodable");
        let ok = decode(&cache, png(4, 4)).expect("a small PNG decodes");
        assert_eq!(ok.size(), Some([4, 4]));
    }

    #[test]
    fn at_most_three_in_flight_failures_are_not_asked_again_and_eviction_spares_what_is_on_show() {
        let mut s = State::sample(0);
        // Not live: fetches go nowhere, so the queue and the answers are driven by hand.
        for u in ["a", "b", "a", "", "c"] {
            want(&mut s, u);
        }
        assert_eq!(s.thumbs.queue, ["a", "b", "c"], "de-duplicated, empty skipped");
        want(&mut s, "d");
        pump(&mut s);
        assert_eq!((s.thumbs.asked.len(), s.thumbs.queue.len()), (IN_FLIGHT, 1), "three asked, one waits");
        s.thumbs.asked.clear();
        s.thumbs.queue.clear();
        s.thumbs.refused.insert("bad".into());
        want(&mut s, "bad");
        assert!(s.thumbs.queue.is_empty(), "a refused URL is not asked again");
        let cache = ImageCache::new(std::sync::Arc::new(|| {}));
        let img = decode(&cache, png(1, 1)).unwrap();
        s.thumbs.shown = ["u0".to_owned()].into();
        for i in 0..=KEPT {
            assert!(arrived(&mut s, format!("u{i}"), Some(img.clone())));
        }
        assert_eq!(s.thumbs.kept.len(), KEPT);
        assert!(get(&s, "u0").is_some(), "on show: kept");
        assert!(get(&s, "u1").is_none(), "the oldest not on show went");
        // A live task draws the picture fetched for its thumbnail; a sample's own cover still wins.
        let mut t = s.tasks[0].clone();
        (t.task_id, t.thumbnail_url) = ("live-task".into(), "u0".into());
        assert!(s.cover(&t).is_some_and(|i| i.size() == Some([1, 1])));
        assert!(!arrived(&mut s, "x".into(), None));
        assert!(s.thumbs.refused.contains("x"));
    }
}
