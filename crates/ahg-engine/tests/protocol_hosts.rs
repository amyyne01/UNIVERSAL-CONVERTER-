//! D36 (Rust only): an `ahg://` link may not point at this PC or the home
//! network. A link the user pastes is their own act and stays allowed.
use ahg_engine::protocol::resolve_protocol_link;
use ahg_engine::types::SourcePlatform;
use ahg_engine::url_detector::detect_url;
use ahg_engine::validate::media_url_arg;

/// (link, accepted through `ahg://`). Every row is accepted when pasted.
const HOSTS: &[(&str, bool)] = &[
    // known platforms and public names
    ("https://www.youtube.com/watch?v=dQw4w9WgXcQ", true),
    ("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC", true),
    ("https://soundcloud.com/artist/track", true),
    ("https://example.com/page", true),
    ("https://example.com/clip.mp4", true),
    ("https://localhost.example.com/a", true),
    ("https://youtu.be/dQw4w9WgXcQ", true),
    ("https://user@www.youtube.com/watch?v=dQw4w9WgXcQ", false),
    ("https://user:pass@www.youtube.com/watch?v=dQw4w9WgXcQ", false),
    // parser differentials: the downloader may read these hosts differently
    (r"http://example.com\@127.0.0.1/a", false),
    (r"http://127.0.0.1\@example.com/a", false),
    ("https://example.com%5c@127.0.0.1/a", false),
    ("https://example.com%5C@127.0.0.1/a", false),
    ("https://example.com@127.0.0.1/x", false),
    ("https://www.youtube.com:8443/watch?v=dQw4w9WgXcQ", true),
    ("https://www.youtube.com@127.0.0.1/watch?v=dQw4w9WgXcQ", false),
    // an inside host with a platform name in the path or query
    ("http://127.0.0.1/youtube.com/watch?v=AAAAAAAAAAA", false),
    ("https://localhost/open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC", false),
    ("http://[::1]/soundcloud.com/a/b", false),
    ("http://10.0.0.5/?u=youtube.com/watch?v=AAAAAAAAAAA", false),
    // localhost
    ("http://localhost/a", false),
    ("http://localhost:8080/a", false),
    ("http://LOCALHOST/a", false),
    ("http://localhost./a", false),
    ("http://app.localhost/a", false),
    ("http://localhost/clip.mp4", false),
    // IPv4 literals, any spelling (public ones too)
    ("http://127.0.0.1/a", false),
    ("http://127.1/a", false),
    ("http://2130706433/a", false),
    ("http://0x7f.0.0.1/a", false),
    ("http://0177.0.0.1/a", false),
    ("http://10.0.0.5/clip.mp4", false),
    ("http://172.16.0.1/a", false),
    ("http://172.31.255.255/a", false),
    ("http://192.168.1.10/a", false),
    ("http://169.254.169.254/latest", false),
    ("http://100.64.0.1/a", false),
    ("http://100.127.255.255/a", false),
    ("http://0.0.0.0/a", false),
    ("http://93.184.216.34/a", false),
    ("http://172.32.0.1/a", false),
    // IPv6 in brackets
    ("http://[::1]/a", false),
    ("http://[::]/a", false),
    ("http://[fe80::1]/a", false),
    ("http://[fc00::1]/a", false),
    ("http://[fd12:3456::1]/a", false),
    ("http://[::ffff:127.0.0.1]/a", false),
    ("http://[2001:db8::1]/clip.mp4", false),
];

/// Never a link, pasted or not (a tab, newline, CR or DEL inside the host);
/// refused through `ahg://` all the same.
const MALFORMED: &[&str] = &[
    "https://exa\tmple.com/x",
    "https://example.com\n.evil/x",
    "https://example.com\r.evil/x",
    "https://example.com\x7f.evil/x",
];

#[test]
fn hosts_through_ahg_links() {
    assert!(!HOSTS.is_empty());
    let mut wrong = Vec::new();
    for (link, accepted) in HOSTS {
        let got = resolve_protocol_link(&format!("ahg://{link}")).is_some();
        if got != *accepted {
            wrong.push(format!("ahg://{link}: accepted={got}, want {accepted}"));
        }
    }
    for link in MALFORMED {
        if resolve_protocol_link(&format!("ahg://{link}")).is_some() {
            wrong.push(format!("ahg://{link:?}: accepted"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Pasted links are not touched by D36: the link checker passes every row, the
/// detector still takes every public row, and the refused rows that the
/// detector reads as a site (the ones D36 changes) are still taken as before.
#[test]
fn the_same_links_pasted_are_still_accepted() {
    assert!(!HOSTS.is_empty());
    let mut inside_and_detected = 0;
    for (link, accepted) in HOSTS {
        assert!(media_url_arg(link).is_ok(), "{link}");
        let platform = detect_url(link).platform;
        if *accepted {
            assert_ne!(platform, SourcePlatform::Unknown, "{link}");
        } else if matches!(platform, SourcePlatform::Generic | SourcePlatform::Direct) {
            inside_and_detected += 1;
        }
    }
    assert_eq!(inside_and_detected, 24);
}
