//! The one place the app makes HTTP requests of its own (yt-dlp makes its own).
//! Blocking calls, made from worker threads. An error status is still a
//! response: each caller decides what a status means.
use std::io::Read;
use std::time::Duration;

use ahg_engine::spotify;
use ureq::tls::{TlsConfig, TlsProvider};
use ureq::{Agent, Proxy, ResponseExt};

pub struct Http {
    proxy: Option<Proxy>,
    /// A proxy was asked for but its address could not be used. Requests are
    /// then refused: going direct would expose the traffic the proxy was for.
    proxy_broken: bool,
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
            None => Self { proxy: None, proxy_broken: false },
            Some(Ok(proxy)) => Self { proxy: Some(proxy), proxy_broken: false },
            Some(Err(_)) => Self { proxy: None, proxy_broken: true },
        }
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
            // Windows' own TLS: nothing extra to ship.
            .tls_config(TlsConfig::builder().provider(TlsProvider::NativeTls).build())
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

/// Spotify's pages and API, read without looking at the status: the engine
/// judges a reply by its content.
pub struct SpotifyHttp {
    http: Http,
}

impl SpotifyHttp {
    pub fn new(http: Http) -> Self {
        Self { http }
    }
}

impl spotify::Transport for SpotifyHttp {
    fn fetch(&mut self, req: &spotify::Request) -> Option<String> {
        self.http.send(req.method, &req.url, &req.headers, req.body.as_deref(), Duration::from_millis(req.timeout_ms))?.text()
    }
}

/// The license registry. Unlike Spotify's replies, a reply here counts only
/// with a success status, and each request has ten seconds. Neither the token
/// in the request nor the reply is ever logged.
#[cfg(feature = "license")]
pub struct GistHttp {
    http: Http,
}

#[cfg(feature = "license")]
impl GistHttp {
    /// Always a direct connection: the proxy in Settings is for media, not for licensing.
    pub fn new() -> Self {
        Self { http: Http::new(None) }
    }
}

#[cfg(feature = "license")]
impl Default for GistHttp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "license")]
impl ahg_engine::license::Registry for GistHttp {
    fn send(&mut self, request: &ahg_engine::license::Request) -> Option<String> {
        let response = self.http.send(request.method, &request.url, &request.headers, request.body.as_deref(), Duration::from_secs(10))?;
        if response.ok() { response.text() } else { None }
    }
}
