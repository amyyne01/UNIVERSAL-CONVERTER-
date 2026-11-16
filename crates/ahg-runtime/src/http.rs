//! The one place the app makes HTTP requests of its own (yt-dlp makes its own).
//! Blocking calls, made from worker threads. An error status is still a
//! response: each caller decides what a status means.
use std::io::Read;
use std::time::Duration;

use ahg_engine::spotify;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::{Agent, Proxy, ResponseExt};

pub struct Http {
    proxy: Option<Proxy>,
    /// A proxy was asked for but its address could not be used. Requests are
    /// then refused: going direct would expose the traffic the proxy was for.
    proxy_broken: bool,
    /// Refuse any plain-http address, the first or after a redirect, and follow
    /// at most 3 redirects (cover art, which comes from addresses a track names).
    strict: bool,
}

pub struct Response {
    pub status: u16,
    /// Where the response finally came from, after any redirects.
    pub final_url: String,
    pub content_length: Option<String>,
    body: ureq::Body,
}

impl Response {
    /// True for a 2xx status.
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The whole body as text, or `None` if it could not be read.
    pub fn text(self) -> Option<String> {
        let mut bytes = Vec::new();
        self.into_reader().read_to_end(&mut bytes).ok()?;
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// The body as a stream, for a download reported chunk by chunk.
    pub fn into_reader(self) -> impl Read {
        self.body.into_with_config().limit(u64::MAX).reader()
    }
}

impl Http {
    /// `proxy` is the address from Settings (`http://`, `https://`,
    /// `socks5://`, `socks5h://`), or `None` or empty for a direct connection.
    /// The address is used exactly as written.
    pub fn new(proxy: Option<&str>) -> Self {
        match proxy.filter(|p| !p.is_empty()).map(Proxy::new) {
            None => Self { proxy: None, proxy_broken: false, strict: false },
            Some(Ok(proxy)) => Self { proxy: Some(proxy), proxy_broken: false, strict: false },
            Some(Err(_)) => Self { proxy: None, proxy_broken: true, strict: false },
        }
    }

    /// This client takes https addresses only and follows at most 3 redirects.
    pub fn https_only(mut self) -> Self {
        self.strict = true;
        self
    }

    /// False when a proxy was asked for and its address could not be used.
    pub fn proxy_usable(&self) -> bool {
        !self.proxy_broken
    }

    /// Send one request. `None` means it failed outright: no connection, a
    /// timeout, a broken reply.
    pub fn send(&self, method: &str, url: &str, headers: &[(String, String)], body: Option<&str>, timeout: Duration) -> Option<Response> {
        if self.proxy_broken {
            return None;
        }
        let agent: Agent = Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(timeout))
            .proxy(self.proxy.clone())
            .https_only(self.strict)
            .max_redirects(if self.strict { 3 } else { 10 })
            // Windows' own TLS and Windows' own list of trusted authorities:
            // nothing extra to ship, and a certificate the system trusts (a
            // company proxy, an antivirus) is trusted here too. The library's
            // built-in list refuses Google's chain.
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build())
            .build()
            .into();
        let mut request = ureq::http::Request::builder().method(method).uri(url);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        let response = agent.run(request.body(body.unwrap_or_default().to_owned()).ok()?).ok()?;
        let status = response.status().as_u16();
        let final_url = response.get_uri().to_string();
        let content_length = response.headers().get("content-length").and_then(|v| v.to_str().ok()).map(str::to_owned);
        Some(Response { status, final_url, content_length, body: response.into_body() })
    }
}

/// YouTube Music's own song search (D34), sent as its web page sends it: no
/// login, no key. `proxy` is asked on every call, so a changed setting applies
/// at once. `None` for anything but a 2xx answer.
pub fn song_search(proxy: impl Fn() -> Option<String> + Send + Sync + 'static) -> crate::download::SongSearch {
    use ahg_engine::downloader::bridge::{SONG_SEARCH_URL, song_search_body};
    std::sync::Arc::new(move |query: &str| {
        let headers = [("Content-Type", "application/json"), ("Origin", "https://music.youtube.com"), ("User-Agent", "Mozilla/5.0")].map(|(k, v)| (k.to_owned(), v.to_owned()));
        let response = Http::new(proxy().as_deref()).send("POST", SONG_SEARCH_URL, &headers, Some(&song_search_body(query)), Duration::from_secs(10))?;
        response.ok().then(|| response.text()).flatten()
    })
}

/// Spotify's pages and API, read without looking at the status: the engine
/// judges a reply by its content.
pub struct SpotifyHttp {
    proxy: Box<dyn Fn() -> Option<String> + Send>,
}

impl SpotifyHttp {
    /// `proxy` is asked on every request, so a changed setting applies at once (D10).
    pub fn new(proxy: impl Fn() -> Option<String> + Send + 'static) -> Self {
        Self { proxy: Box::new(proxy) }
    }
}

impl spotify::Transport for SpotifyHttp {
    fn fetch(&mut self, req: &spotify::Request) -> Option<String> {
        Http::new((self.proxy)().as_deref()).send(req.method, &req.url, &req.headers, req.body.as_deref(), Duration::from_millis(req.timeout_ms))?.text()
    }
}

/// One cover-art request (`Core::fetch_cover`). `proxy` is the setting as it is
/// right now (D10). The caller limits how much of the body it reads.
pub fn cover_get(url: &str, proxy: Option<&str>) -> Option<crate::engine_update::Fetched> {
    let headers = [("User-Agent".to_owned(), "Mozilla/5.0".to_owned())];
    let response = Http::new(proxy).https_only().send("GET", url, &headers, None, Duration::from_secs(20))?;
    let (status, final_url, content_length) = (response.status, response.final_url.clone(), response.content_length.clone());
    Some(crate::engine_update::Fetched { status, final_url, content_length, body: Box::new(response.into_reader()) })
}

/// The updaters' requests to GitHub's release pages. Always direct: update
/// checks do not use the proxy (D10). There is no limit on the whole
/// download, only on how long the connection may stay silent, so a large file
/// on a slow line is not cut off while it is still arriving.
pub struct Releases;

impl crate::engine_update::Fetch for Releases {
    fn get(&self, request: &ahg_engine::updater::Request) -> Option<crate::engine_update::Fetched> {
        const QUIET: Duration = Duration::from_secs(30);
        let agent: Agent = Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(QUIET))
            .timeout_recv_response(Some(QUIET))
            .timeout_per_read(Some(QUIET))
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).root_certs(RootCerts::PlatformVerifier).build())
            .build()
            .into();
        let response = agent.get(&request.url).header("User-Agent", request.user_agent).call().ok()?;
        let status = response.status().as_u16();
        let final_url = response.get_uri().to_string();
        let content_length = response.headers().get("content-length").and_then(|v| v.to_str().ok()).map(str::to_owned);
        Some(crate::engine_update::Fetched { status, final_url, content_length, body: Box::new(response.into_body().into_with_config().limit(u64::MAX).reader()) })
    }
}
