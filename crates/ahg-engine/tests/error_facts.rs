//! Facts about a failure code the interface needs: is it permanent, what fix to offer.
//! Cases transcribed by hand from electron src/test/lib/errors.test.ts ("marks exactly the
//! codes no retry can fix as permanent") and the FIX table in src/lib/errors.ts (Electron has
//! no test for fixFor, so its expectations come from that table).
use ahg_engine::downloader::errors::{Fix, fix_for, is_permanent};
use ahg_engine::types::ErrorCode::{self, *};

// ERROR_CODES from shared/types.ts
const ALL: [ErrorCode; 12] =
    [Private, Removed, AgeRestricted, LoginRequired, GeoBlocked, Drm, RateLimited, Network, Unsupported, FfmpegMissing, DiskFull, Unknown];

#[test]
fn marks_exactly_the_codes_no_retry_can_fix_as_permanent() {
    assert!(!ALL.is_empty());
    let mut permanent: Vec<_> = ALL.into_iter().filter(|c| is_permanent(*c)).collect();
    permanent.sort_by_key(|c| format!("{c:?}"));
    // ['drm', 'geo_blocked', 'removed', 'unsupported']
    assert_eq!(permanent, [Drm, GeoBlocked, Removed, Unsupported]);
}

#[test]
fn offers_a_fix_only_where_electron_does() {
    let want = [
        (Private, Some(Fix::Settings)),
        (AgeRestricted, Some(Fix::Settings)),
        (LoginRequired, Some(Fix::Settings)),
        (DiskFull, Some(Fix::Settings)),
        (Unknown, Some(Fix::Engine)),
        (Removed, None),
        (GeoBlocked, None),
        (Drm, None),
        (RateLimited, None),
        (Network, None),
        (Unsupported, None),
        (FfmpegMissing, None),
    ];
    assert_eq!(want.len(), ALL.len());
    for (code, fix) in want {
        assert_eq!(fix_for(code), fix, "{code:?}");
    }
}
