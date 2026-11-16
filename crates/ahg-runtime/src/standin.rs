//! Stand-ins for everything that reaches outside the app, for tests (this
//! crate's and the interface's). Compiled only with the `standin` feature.
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

use ahg_engine::spotify;
use ahg_engine::updater::Request;

use crate::core::Outside;
use crate::desktop::Desktop;
use crate::engine_update::{Fetch, Fetched};

/// Url -> (status, final url, body).
type Replies = HashMap<String, (u16, String, Vec<u8>)>;

/// A scripted network for the updaters: it answers from a table and
/// remembers what it was asked.
#[derive(Clone, Default)]
pub struct Net {
    replies: Arc<Mutex<Replies>>,
    asked: Arc<Mutex<Vec<String>>>,
}

impl Net {
    pub fn with(self, url: &str, status: u16, body: &[u8]) -> Self {
        self.replies.lock().unwrap().insert(url.to_owned(), (status, url.to_owned(), body.to_vec()));
        self
    }

    /// `url` answers, but from `final_url`.
    pub fn redirected(self, url: &str, final_url: &str, body: &[u8]) -> Self {
        self.replies.lock().unwrap().insert(url.to_owned(), (200, final_url.to_owned(), body.to_vec()));
        self
    }

    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

impl Fetch for Net {
    fn get(&self, request: &Request) -> Option<Fetched> {
        assert_eq!(request.user_agent, "AHG-Universal-Converter");
        self.asked.lock().unwrap().push(request.url.clone());
        let (status, final_url, body) = self.replies.lock().unwrap().get(&request.url)?.clone();
        Some(Fetched { status, final_url, content_length: Some(body.len().to_string()), location: None, body: Box::new(Cursor::new(body)) })
    }
}

/// A desktop that shows and opens nothing.
pub struct Quiet;

impl Desktop for Quiet {
    fn reveal(&self, _: &str) {}

    fn open(&self, _: &str) -> bool {
        false
    }
}

/// Spotify, unreachable.
struct Offline;

impl spotify::Transport for Offline {
    fn fetch(&mut self, _: &spotify::Request) -> Option<String> {
        None
    }
}

impl Outside {
    /// A complete `Outside` that reaches nothing: no song is found, Spotify and
    /// the cover host cannot be reached, no release exists, no file is shown.
    /// Swap a field for what a test needs.
    pub fn standin() -> Self {
        Self { song_search: Arc::new(|_: &str| None), spotify: Box::new(Offline), releases: Arc::new(Net::default()), desktop: Box::new(Quiet), cover: Box::new(|_, _| None) }
    }
}
