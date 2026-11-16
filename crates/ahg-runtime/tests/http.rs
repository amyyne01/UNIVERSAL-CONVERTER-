//! The HTTP layer against a small server on this machine. Nothing leaves it.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

use ahg_engine::spotify::{Request as SpotifyRequest, Transport};
use ahg_runtime::http::{Http, SpotifyHttp};

/// What the test server saw of one request.
#[derive(Debug)]
struct Seen {
    line: String,
    headers: Vec<String>,
    body: String,
}

/// The address of a port nothing listens on: bound here, then released.
fn closed_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().to_string()
}

fn closed_url(path: &str) -> String {
    format!("http://{}{path}", closed_addr())
}

/// A server that answers each request by its path, and reports what it saw.
fn server() -> (String, Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = channel();
    let origin = base.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let tx = tx.clone();
            let origin = origin.clone();
            thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    return;
                }
                let mut headers = Vec::new();
                let mut length = 0;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = h.to_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                    headers.push(h.trim().to_owned());
                }
                let mut body = vec![0u8; length];
                let _ = reader.read_exact(&mut body);
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let _ = tx.send(Seen { line: line.trim().to_owned(), headers, body: String::from_utf8_lossy(&body).into_owned() });
                let (status, extra, payload) = match path.as_str() {
                    "/ok" => ("200 OK", String::new(), "hello \u{e9}".to_owned()),
                    "/error" => ("500 Internal Server Error", String::new(), "{\"errors\":[]}".to_owned()),
                    "/moved" => ("302 Found", format!("Location: {origin}/ok\r\n"), String::new()),
                    "/slow" => {
                        thread::sleep(Duration::from_secs(5));
                        ("200 OK", String::new(), "late".to_owned())
                    }
                    "/big" => ("200 OK", String::new(), "x".repeat(300_000)),
                    _ => ("404 Not Found", String::new(), "nope".to_owned()),
                };
                let _ = write!(stream, "HTTP/1.1 {status}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len());
            });
        }
    });
    (base, rx)
}

const T: Duration = Duration::from_secs(10);

#[test]
fn a_get_returns_the_status_the_final_url_and_the_body() {
    let (base, seen) = server();
    let http = Http::new(None);
    let res = http.send("GET", &format!("{base}/ok"), &[("X-Test".into(), "yes".into())], None, T).unwrap();
    assert_eq!((res.status, res.ok()), (200, true));
    assert_eq!(res.final_url, format!("{base}/ok"));
    assert_eq!(res.content_length.as_deref(), Some("8")); // bytes, and the accented letter is two
    assert_eq!(res.text().unwrap(), "hello \u{e9}");
    let seen = seen.recv().unwrap();
    assert!(seen.line.starts_with("GET /ok "));
    assert!(seen.headers.iter().any(|h| h.eq_ignore_ascii_case("x-test: yes")), "{:?}", seen.headers);
}

#[test]
fn an_error_status_is_a_response_not_a_failure() {
    let (base, _seen) = server();
    let res = Http::new(None).send("GET", &format!("{base}/error"), &[], None, T).unwrap();
    assert_eq!((res.status, res.ok()), (500, false));
    assert_eq!(res.text().unwrap(), "{\"errors\":[]}");
}

#[test]
fn a_redirect_is_followed_and_the_final_address_reported() {
    let (base, _seen) = server();
    let res = Http::new(None).send("GET", &format!("{base}/moved"), &[], None, T).unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(res.final_url, format!("{base}/ok"));
}

#[test]
fn a_body_is_sent_with_a_patch() {
    let (base, seen) = server();
    let headers = vec![("Content-Type".to_owned(), "application/json".to_owned())];
    let res = Http::new(None).send("PATCH", &format!("{base}/ok"), &headers, Some("{\"a\":1}"), T).unwrap();
    assert_eq!(res.status, 200);
    let seen = seen.recv().unwrap();
    assert!(seen.line.starts_with("PATCH /ok "));
    assert_eq!(seen.body, "{\"a\":1}");
}

#[test]
fn a_request_that_takes_too_long_fails_at_the_timeout() {
    let (base, _seen) = server();
    let started = Instant::now();
    assert!(Http::new(None).send("GET", &format!("{base}/slow"), &[], None, Duration::from_millis(600)).and_then(|r| r.text()).is_none());
    assert!(started.elapsed() < Duration::from_secs(4));
}

#[test]
fn nothing_listening_is_a_failure_not_a_panic() {
    assert!(Http::new(None).send("GET", &closed_url("/none"), &[], None, Duration::from_secs(3)).is_none());
}

#[test]
fn a_large_body_can_be_read_in_chunks() {
    let (base, _seen) = server();
    let res = Http::new(None).send("GET", &format!("{base}/big"), &[], None, T).unwrap();
    let mut reader = res.into_reader();
    let (mut total, mut reads, mut buf) = (0, 0, [0u8; 16_384]);
    loop {
        let n = reader.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        total += n;
        reads += 1;
    }
    assert_eq!(total, 300_000);
    assert!(reads > 1);
}

#[test]
fn the_spotify_transport_returns_the_text_whatever_the_status() {
    let (base, seen) = server();
    let mut transport = SpotifyHttp::new(|| None);
    let request = |path: &str| SpotifyRequest {
        url: format!("{base}{path}"),
        method: "GET",
        headers: vec![("User-Agent".into(), "test-agent".into())],
        body: None,
        timeout_ms: 5000,
    };
    assert_eq!(transport.fetch(&request("/ok")).as_deref(), Some("hello \u{e9}"));
    assert!(seen.recv().unwrap().headers.iter().any(|h| h.eq_ignore_ascii_case("user-agent: test-agent")));
    // Spotify's replies are read without looking at the status.
    assert_eq!(transport.fetch(&request("/error")).as_deref(), Some("{\"errors\":[]}"));
    assert_eq!(transport.fetch(&SpotifyRequest { url: closed_url("/x"), ..request("/ok") }), None);
}

/// The Rust Spotify reader against the live service: a public playlist and a
/// track, with no credentials. Needs the network, so it only runs
/// when asked: `cargo test -p ahg-runtime --test http -- --ignored`.
#[test]
#[ignore = "reads public Spotify data; needs the network"]
fn the_real_spotify_is_read_without_credentials() {
    use ahg_engine::spotify::{Spotify, system_now_ms};
    let mut spotify = Spotify::new(SpotifyHttp::new(|| None), Box::new(system_now_ms));

    let playlist = spotify.fetch_playlist("37i9dQZF1DWXRqgorJj26U").ok().flatten().expect("the playlist should load");
    assert!(!playlist.name.is_empty());
    assert!(playlist.tracks.len() > 50, "only {} tracks", playlist.tracks.len());
    let first = &playlist.tracks[0];
    assert!(!first.name.is_empty() && !first.artist.is_empty() && first.duration_ms > 30_000.0, "{first:?}");

    let track = spotify.fetch_track(&format!("https://open.spotify.com/track/{}", first.id)).ok().flatten().expect("the track should load");
    assert_eq!(track.name, first.name);

    // Search is not asserted: on 2026-11-15 Spotify's search answered nothing to the
    // Electron app either (QUIRKS.md, Q6). It must at least not fail.
    let _ = spotify.search(&format!("{} {}", first.artist, first.name));
}

#[test]
fn a_proxy_that_cannot_be_used_stops_requests_instead_of_going_direct() {
    let (base, _seen) = server();
    // The setting asks for a proxy. If it cannot be understood, connecting
    // directly would expose the traffic the proxy was meant to carry.
    let http = Http::new(Some("not a proxy address"));
    assert!(!http.proxy_usable());
    assert!(http.send("GET", &format!("{base}/ok"), &[], None, T).is_none());
    // No proxy asked for, or an empty setting, is a direct connection.
    assert!(Http::new(None).proxy_usable());
    assert!(Http::new(Some("")).send("GET", &format!("{base}/ok"), &[], None, T).is_some());
}

#[test]
fn every_proxy_scheme_settings_accept_is_understood_as_written() {
    // `socks5h` means the proxy resolves host names. It must reach the client
    // unchanged: turning it into `socks5` would resolve names on this machine.
    for address in ["http://127.0.0.1:9", "https://127.0.0.1:9", "socks5://127.0.0.1:9", "socks5h://127.0.0.1:9"] {
        assert!(Http::new(Some(address)).proxy_usable(), "{address}");
    }
}

#[test]
fn requests_go_through_the_proxy_when_one_is_set() {
    // Nothing listens on the proxy's port, so a request can only fail if it really went to the proxy.
    let (base, _seen) = server();
    let http = Http::new(Some(&format!("http://{}", closed_addr())));
    assert!(http.send("GET", &format!("{base}/ok"), &[], None, Duration::from_secs(3)).is_none());
}

/// The real YouTube Music song search: it answers without credentials, and
/// its rows carry what the bridge matches on. Network; run with --ignored.
#[test]
#[ignore = "talks to the real YouTube Music"]
fn the_real_song_search_returns_rows_with_artist_and_length() {
    use ahg_engine::downloader::bridge::song_rows;
    let search = ahg_runtime::http::song_search(|| None);
    let rows = song_rows(&search("Fleetwood Mac - Dreams").expect("the search should answer"));
    assert!(rows.len() >= 5, "{} rows", rows.len());
    let first = &rows[0];
    assert!(first.title.contains("Dreams") && first.artist.contains("Fleetwood Mac") && first.duration.is_some(), "{first:?}");
}

/// D34 against the real Spotify: playlist rows and single tracks both state
/// their explicit flag. Network; run with --ignored.
#[test]
#[ignore = "talks to the real Spotify"]
fn the_real_spotify_states_which_tracks_are_explicit() {
    use ahg_engine::spotify::{Spotify, system_now_ms};
    let mut spotify = Spotify::new(SpotifyHttp::new(|| None), Box::new(system_now_ms));
    // A rap playlist: it holds explicit tracks and clean ones.
    let playlist = spotify.fetch_playlist("37i9dQZF1DX0XUsuxWHRQd").ok().flatten().expect("the playlist should load");
    let stated = playlist.tracks.iter().filter(|t| t.explicit.is_some()).count();
    let explicit: Vec<_> = playlist.tracks.iter().filter(|t| t.explicit == Some(true)).collect();
    println!("{} tracks, {stated} state the flag, {} explicit", playlist.tracks.len(), explicit.len());
    assert!(stated * 10 >= playlist.tracks.len() * 9, "most rows should state the flag: {stated} of {}", playlist.tracks.len());
    assert!(!explicit.is_empty(), "a rap playlist has explicit tracks");
    // A piano playlist: the flag is stated there too, and says no.
    let calm = spotify.fetch_playlist("37i9dQZF1DX4sWSpwq3LiO").ok().flatten().expect("the playlist should load");
    let clean = calm.tracks.iter().filter(|t| t.explicit == Some(false)).count();
    println!("{} calm tracks, {clean} stated as not explicit", calm.tracks.len());
    assert!(clean * 10 >= calm.tracks.len() * 9, "{clean} of {}", calm.tracks.len());
    // An album goes through a different response: Kendrick Lamar, "DAMN.".
    let album = spotify.fetch_album("4eLPsYPBmXABThSJ821sqY").ok().flatten().expect("the album should load");
    let (stated, dirty) = (album.tracks.iter().filter(|t| t.explicit.is_some()).count(), album.tracks.iter().filter(|t| t.explicit == Some(true)).count());
    println!("album: {} tracks, {stated} state the flag, {dirty} explicit", album.tracks.len());
    assert!(stated == album.tracks.len() && dirty > 0, "{stated} of {} stated, {dirty} explicit", album.tracks.len());
    // The same track read on its own says the same.
    let alone = spotify.fetch_track(&format!("https://open.spotify.com/track/{}", explicit[0].id)).ok().flatten().expect("the track should load");
    assert_eq!(alone.explicit, Some(true), "{} read alone", alone.name);
}

#[test]
fn a_proxy_change_reaches_the_very_next_spotify_request() {
    let (base, _seen) = server();
    let proxy = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let mut transport = SpotifyHttp::new({
        let proxy = proxy.clone();
        move || proxy.lock().unwrap().clone()
    });
    let request = SpotifyRequest { url: format!("{base}/ok"), method: "GET", headers: vec![], body: None, timeout_ms: 3000 };
    assert!(transport.fetch(&request).is_some());
    // Nothing listens at the proxy, so the request can only fail by going there.
    *proxy.lock().unwrap() = Some(format!("http://{}", closed_addr()));
    assert_eq!(transport.fetch(&request), None);
    *proxy.lock().unwrap() = None;
    assert!(transport.fetch(&request).is_some());
}
