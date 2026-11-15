//! A scripted network for the updater tests: it answers from a table and
//! remembers what it was asked.
#![allow(dead_code)]
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

use ahg_engine::updater::Request;
use ahg_runtime::engine_update::{Fetch, Fetched};
use sha2::{Digest, Sha256};

pub fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Url -> (status, final url, body).
type Replies = HashMap<String, (u16, String, Vec<u8>)>;

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
        Some(Fetched { status, final_url, content_length: Some(body.len().to_string()), body: Box::new(Cursor::new(body)) })
    }
}
