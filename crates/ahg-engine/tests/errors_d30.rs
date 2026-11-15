//! D30: the classifier matches the phrases yt-dlp really prints, not loose words.
//! Real-looking lines are built from phrases quoted from yt-dlp's source (see the
//! comment block in src/downloader/errors.rs); stray words must no longer flip the verdict.
use ahg_engine::downloader::errors::{ErrorClass, classify_error, error_code_of, explain_error};
use ahg_engine::types::ErrorCode;

#[track_caller]
fn expect(line: &str, class: ErrorClass, code: ErrorCode) {
    assert_eq!(classify_error(line), class, "class of {line}");
    assert_eq!(error_code_of(line), code, "code of {line}");
}

#[test]
fn geo_phrases_still_classify_as_geo() {
    for line in [
        "ERROR: [soundcloud] 123: This video is not available from your location due to geo restriction",
        "ERROR: [youtube] abc: The uploader has not made this video available in your country",
        "ERROR: [twitter] 1: This video is not available from your location due to geo restriction\n\
         You might want to use a VPN or a proxy server (with --proxy) to workaround.",
        "[youtube] abc: Video is geo restricted. Retrying extraction with fake IP 1.2.3.4 (DE) as X-Forwarded-For.",
        "ERROR: [youtube:tab] This playlist is likely not available in your region.",
        "ERROR: This video is not available in your country",
    ] {
        expect(line, ErrorClass::Geo, ErrorCode::GeoBlocked);
        assert_eq!(explain_error(line), "That post is not available in your region.");
    }
}

#[test]
fn private_and_sign_in_phrases_still_classify_as_auth() {
    for line in [
        "ERROR: [youtube] abc: Private video. Sign in if you've been granted access to this video",
        "ERROR: [youtube:tab] This playlist is private",
        "ERROR: [TikTok] 7: This user's account is private. Log into an account that has access",
        "ERROR: [TikTok] u: This user's account is likely either private or all of their videos are private. Log into an account that has access",
    ] {
        expect(line, ErrorClass::Auth, ErrorCode::Private);
        assert!(explain_error(line).starts_with("That post needs a sign-in"));
    }
    for line in [
        "ERROR: [youtube] abc: Sign in to confirm you\u{2019}re not a bot. Use --cookies-from-browser or --cookies for the authentication.",
        "ERROR: [twitch:stream] You must be logged into an account that has access",
    ] {
        expect(line, ErrorClass::Auth, ErrorCode::LoginRequired);
    }
    for line in ["ERROR: [youtube] abc: Private video", "[youtube] abc: Downloading webpage\nERROR: [youtube] abc: Private video\n"] {
        expect(line, ErrorClass::Auth, ErrorCode::Private);
    }
    expect(
        "ERROR: [youtube] abc: Sign in to confirm your age. This video may be inappropriate for some users.",
        ErrorClass::Auth,
        ErrorCode::AgeRestricted,
    );
}

#[test]
fn removed_drm_and_not_found_phrases_stay_permanent() {
    for line in [
        "ERROR: [youtube] abc: This video has been removed for violating YouTube's policy on hate speech.",
        "ERROR: [twitter] 1: Tweet has been deleted",
        "ERROR: [twitch:stream] Channel \"someone\" not found",
        "ERROR: [twitter:spaces] 1: Twitter Space not found",
        "ERROR: [youtube] abc: Video unavailable",
        "ERROR: [youtube] abc: This video is not available.",
        "ERROR: [youtube] abc: Video blocked on copyright grounds",
        "ERROR: [x] 1: Video not found",
    ] {
        expect(line, ErrorClass::Permanent, ErrorCode::Removed);
    }
    for line in [
        "ERROR: [vimeo] 9: This video is DRM protected",
        "ERROR: [generic] This video is DRM protected",
        "ERROR: Unsupported URL: https://x.example/v. The requested site is known to use DRM protection. It will not be supported.",
    ] {
        expect(line, ErrorClass::Permanent, ErrorCode::Drm);
    }
}

#[test]
fn network_phrases_stay_retryable() {
    for line in [
        "ERROR: [generic] A network error has occurred.",
        "ERROR: [generic] Unable to download webpage: Connection to host timed out. (connect timeout=20.0)",
        "ERROR: [youtube] abc: Unable to download webpage: HTTP Error 503: Service Temporarily Unavailable",
        "ERROR: unable to download video data: HTTP Error 502: Bad Gateway",
        "ERROR: [generic] Read timeout while fetching the page",
        "ERROR: [generic] Temporary failure in name resolution",
    ] {
        expect(line, ErrorClass::Retryable, ErrorCode::Network);
    }
}

/// A write/convert failure that merely mentions a stray word is not geo, auth,
/// removed, DRM or retryable: it is a permanent error shown with yt-dlp's own text.
#[test]
fn stray_words_no_longer_flip_the_verdict() {
    for line in [
        r"ERROR: unable to open for writing: [Errno 13] Permission denied: 'C:\Users\x\Private Videos\a.mp3'",
        r"ERROR: Unable to create directory: [WinError 5] Access is denied: 'D:\Music\Region Mix'",
        "ERROR: Postprocessing: Conversion failed! Channel GeoWizard - Live Set.mp3",
        r"ERROR: unable to open for writing: [Errno 22] Invalid argument: 'George Michael - Careless Whisper.mp3'",
        r"ERROR: Unable to create directory: [WinError 5] Access is denied: 'D:\Regional Hits'",
        r"ERROR: unable to open for writing: [Errno 13] Permission denied: 'C:\Drm Talk\a.mp3'",
        r"ERROR: unable to open for writing: [Errno 13] Permission denied: 'C:\Private Video.mp3'",
    ] {
        expect(line, ErrorClass::Permanent, ErrorCode::Unknown);
        assert!(explain_error(line).starts_with("The download stopped with an error:"), "{line}");
    }
}
