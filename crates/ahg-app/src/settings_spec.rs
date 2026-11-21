//! The settings sections and values every preset draws, as data. Wording only from `copy`, values read from
//! `State.settings` and `State.plan`; a preset draws rows and calls `actions::set_setting` with a row's key.
use ahg_engine::tier::{self, BASIC_LIMITS, MAX_CONCURRENT_DOWNLOADS, Plan};
use ahg_engine::types::VideoQuality;
use serde_json::{Value, json};

use crate::copy::{found, settings as c};
use crate::gate::{PRESETS, State};

/// One choice of a select; `lock` is the sentence a tap on it says on Basic.
#[derive(Debug, Clone, PartialEq)]
pub struct Opt {
    pub value: String,
    pub label: String,
    pub lock: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Toggle,
    Select(Vec<Opt>),
    Text,
    Time,
    /// Weekday chips: `copy::settings::DAYS` and `DAYS_SHORT`, values 0 (Sunday) to 6.
    Days,
    /// `copy::settings::THEMES`, values light, dark, system.
    Theme,
    /// The ids of `ahg_engine::config::PALETTE_IDS`.
    Palette,
    /// The preset picker; the value is the preset's name.
    Layout,
    Folder,
    /// A line with a button (updates, plan, help, cookies import) or none.
    Status,
    /// A two-step confirm: `Row::armed` is (the armed description, the confirm button).
    Danger,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The config key it writes; `None` for rows that are not a setting. `layout` and `holdUnconfirmed` are Rust-only.
    pub key: Option<&'static str>,
    pub label: &'static str,
    pub desc: String,
    pub kind: Kind,
    pub value: Value,
    pub placeholder: Option<&'static str>,
    pub button: Option<&'static str>,
    pub armed: Option<(&'static str, &'static str)>,
    /// Basic only: the whole row is locked, with this sentence.
    pub lock: Option<String>,
    /// Shown but inactive (subtitle rows with Subtitles off, schedule rows with the scheduler off).
    pub dimmed: bool,
}

impl Row {
    /// Locked as a whole, or with a locked choice, on this plan.
    pub fn gated(&self) -> bool {
        self.lock.is_some() || matches!(&self.kind, Kind::Select(o) if o.iter().any(|o| o.lock.is_some()))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// Empty for Destination, which has no heading in Electron.
    pub title: &'static str,
    pub hint: Option<&'static str>,
    pub rows: Vec<Row>,
}

fn row(key: Option<&'static str>, (label, desc): (&'static str, &'static str), kind: Kind, value: Value) -> Row {
    Row { key, label, desc: desc.into(), kind, value, placeholder: None, button: None, armed: None, lock: None, dimmed: false }
}

fn select(values: &[&str], labels: &[&str], lock: impl Fn(&str, &str) -> Option<String>) -> Kind {
    Kind::Select(values.iter().zip(labels).map(|(v, l)| Opt { value: (*v).into(), label: (*l).into(), lock: lock(v, l) }).collect())
}

fn premium_video(v: &str) -> bool {
    serde_json::from_value::<VideoQuality>(json!(v)).is_ok_and(tier::is_premium_video_quality)
}

pub fn sections(s: &State) -> Vec<Section> {
    let basic = s.plan == Plan::Basic;
    let get = |k: &str| s.settings.get(k).cloned().unwrap_or(Value::Null);
    let text = |k: &str| s.settings.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    let on = |k: &str| s.settings.get(k).and_then(Value::as_bool).unwrap_or(false);
    let toggle = |k: &'static str, l: (&'static str, &'static str)| row(Some(k), l, Kind::Toggle, json!(on(k)));
    let pick = |k: &'static str, label: &'static str, values: &[&str], labels: &[&str]| {
        row(Some(k), (label, ""), select(values, labels, |_, _| None), get(k))
    };
    let input = |k: &'static str, l: (&'static str, &'static str), ph: &'static str| {
        let mut r = row(Some(k), l, Kind::Text, get(k));
        r.placeholder = Some(ph);
        r
    };

    let mut destination = row(Some("outputDir"), (c::SAVING_TO, ""), Kind::Folder, get("outputDir"));
    destination.button = Some(c::CHANGE);

    // Download defaults: on Basic the value shown is what the plan would really run.
    let format = row(
        Some("defaultFormat"),
        (c::AUDIO_FORMAT, ""),
        select(&["mp3", "aac", "m4a", "ogg", "opus", "flac", "wav", "alac"], &found::AUDIO_FORMATS, |v, l| {
            (basic && tier::is_premium_format(v)).then(|| found::lock_format(l))
        }),
        json!(tier::clamp_format(&text("defaultFormat"), s.plan)),
    );
    let quality = row(
        Some("defaultQuality"),
        (c::AUDIO_QUALITY, ""),
        select(&["128", "192", "256", "320", "lossless"], &found::AUDIO_QUALITIES, |v, _| {
            (basic && tier::clamp_audio_quality(v, Plan::Basic) != v).then(|| found::LOCK_LOSSLESS_QUALITY.to_owned())
        }),
        json!(tier::clamp_audio_quality(&text("defaultQuality"), s.plan)),
    );
    let video_now = text("defaultVideoQuality");
    let video = row(
        Some("defaultVideoQuality"),
        (c::VIDEO_QUALITY, ""),
        select(&["360p", "480p", "720p", "1080p", "2160p", "best"], &found::VIDEO_QUALITIES, |v, _| {
            (basic && premium_video(v)).then(|| found::lock_video_quality(v))
        }),
        match serde_json::from_value::<VideoQuality>(json!(video_now)) {
            Ok(q) => json!(tier::clamp_video_quality(q, s.plan)),
            Err(_) => json!(video_now),
        },
    );
    let slots: Vec<String> = (1..=MAX_CONCURRENT_DOWNLOADS).map(|n| n.to_string()).collect();
    let slot_labels: Vec<String> = (1..=MAX_CONCURRENT_DOWNLOADS as usize).map(c::concurrency_option).collect();
    let mut concurrency = row(
        Some("maxConcurrentDownloads"),
        c::SIMULTANEOUS,
        select(
            &slots.iter().map(String::as_str).collect::<Vec<_>>(),
            &slot_labels.iter().map(String::as_str).collect::<Vec<_>>(),
            |v, _| (basic && v.parse::<u32>().is_ok_and(|n| n > BASIC_LIMITS.max_concurrent)).then(|| c::LOCK_CONCURRENCY.to_owned()),
        ),
        json!(tier::clamp_concurrency(get("maxConcurrentDownloads").as_f64().unwrap_or(f64::NAN), s.plan)),
    );
    if basic {
        concurrency.desc = c::SIMULTANEOUS_BASIC.into();
    }
    let mut review = toggle("holdUnconfirmed", c::NEEDS_REVIEW);
    review.value = json!(s.own.hold_unconfirmed);
    let defaults = vec![
        format,
        quality,
        video,
        toggle("skipExisting", c::SKIP_EXISTING),
        toggle("embedThumbnail", c::EMBED_THUMBNAIL),
        toggle("embedMetadata", c::EMBED_METADATA),
        concurrency,
        review,
    ];

    // More download options
    let subtitles_off = text("subtitleMode") == "off";
    let mut langs = input("subtitleLangs", (c::SUBTITLE_LANGUAGES, ""), c::SUBTITLE_LANGUAGES_PLACEHOLDER);
    langs.dimmed = subtitles_off;
    langs.lock = (basic && BASIC_LIMITS.max_subtitle_langs < 2).then(|| c::LOCK_SUBTITLE_LANGUAGES.to_owned());
    let mut auto = toggle("subtitleAuto", c::AUTO_CAPTIONS);
    auto.dimmed = subtitles_off;
    auto.lock = (basic && !BASIC_LIMITS.subtitle_auto).then(|| c::LOCK_SUBTITLE_AUTO.to_owned());
    let no_cookies = basic && !BASIC_LIMITS.cookies;
    let cookies = row(
        Some("cookieBrowser"),
        (c::SIGN_IN_WITH, ""),
        select(&["", "chrome", "edge", "firefox", "brave", "chromium", "opera", "vivaldi"], &c::COOKIE_BROWSERS, |v, _| {
            (no_cookies && !v.is_empty()).then(|| c::LOCK_COOKIES.to_owned())
        }),
        json!(if no_cookies { String::new() } else { text("cookieBrowser") }),
    );
    let mut import = row(None, c::IMPORT_COOKIES, Kind::Status, Value::Null);
    import.button = Some(c::CHOOSE_FILE);
    let mut split = toggle("splitChapters", c::SPLIT_BY_CHAPTER);
    split.lock = (basic && !BASIC_LIMITS.split_chapters).then(|| c::LOCK_SPLIT_CHAPTERS.to_owned());
    let more = vec![
        pick("subtitleMode", c::SUBTITLES, &["off", "embed", "file", "both"], &c::SUBTITLE_MODES),
        langs,
        pick("sponsorBlock", c::SPONSOR_SEGMENTS, &["off", "mark", "remove"], &c::SPONSOR_MODES),
        pick("videoContainer", c::VIDEO_CONTAINER, &["mp4", "mkv", "webm"], &c::CONTAINERS),
        pick("videoCodec", c::VIDEO_CODEC, &["any", "h264", "vp9", "av1"], &c::CODECS),
        cookies,
        import,
        auto,
        toggle("embedChapters", c::EMBED_CHAPTERS),
        split,
        toggle("writeThumbnail", c::SAVE_COVER_ART),
        input("outputTemplate", c::FILE_NAMING, c::FILE_NAMING_PLACEHOLDER),
    ];

    let mut layout = row(Some("layout"), c::LAYOUT, Kind::Layout, json!(PRESETS[s.own.preset % PRESETS.len()].0));
    layout.button = Some(c::LAYOUT_CHOOSE);
    let mut setup = row(None, crate::copy::onboarding::RUN_AGAIN, Kind::Status, Value::Null);
    setup.button = Some(crate::copy::onboarding::RUN_AGAIN_BUTTON);
    let appearance = vec![
        row(Some("theme"), c::THEME, Kind::Theme, get("theme")),
        row(Some("palette"), c::PALETTE, Kind::Palette, get("palette")),
        toggle("clipboardWatch", c::CLIPBOARD_WATCHER),
        toggle("showNotifications", c::NOTIFICATIONS),
        toggle("discordRichPresence", c::DISCORD),
        layout,
        setup,
    ];

    // Scheduled downloads: the master toggle reads off on Basic whatever the file says.
    let scheduled = !basic && on("scheduleEnabled");
    let mut enable = row(Some("scheduleEnabled"), (c::ENABLE_SCHEDULER, ""), Kind::Toggle, json!(scheduled));
    enable.lock = (basic && !BASIC_LIMITS.scheduler).then(|| c::LOCK_SCHEDULER.to_owned());
    let mut time = row(Some("scheduleTime"), c::START_TIME, Kind::Time, get("scheduleTime"));
    let mut days = row(Some("scheduleDays"), c::ACTIVE_DAYS, Kind::Days, get("scheduleDays"));
    let mut shutdown = toggle("scheduleShutdown", c::SHUTDOWN);
    if on("scheduleShutdown") {
        shutdown.desc = c::SHUTDOWN_ON.into();
    }
    for r in [&mut time, &mut days, &mut shutdown] {
        r.dimmed = !scheduled;
    }
    let schedule = vec![enable, time, days, shutdown];

    // Software: the status lines come from State.update / State.engine, drawn by the preset.
    let mut app = row(None, (c::APP_NAME, c::APP_REST), Kind::Status, Value::Null);
    app.button = Some(c::CHECK_FOR_UPDATES);
    let mut engine = row(None, (c::ENGINE_NAME, c::ENGINE_IDLE), Kind::Status, Value::Null);
    engine.button = Some(c::ENGINE_CHECK);
    let software = vec![app, engine, toggle("autoUpdateEngine", c::AUTO_UPDATE_ENGINE)];

    let advanced = vec![input("rateLimit", c::RATE_LIMIT, c::RATE_LIMIT_PLACEHOLDER), input("proxy", c::PROXY, c::PROXY_PLACEHOLDER), {
        // Shown as it reads on a keyboard (`CommandOrControl` is Ctrl), like the keycaps on Home.
        let mut r = input("globalHotkey", c::GLOBAL_HOTKEY, c::GLOBAL_HOTKEY_PLACEHOLDER);
        r.value = json!(crate::format::hotkey(&text("globalHotkey")));
        r
    }];

    // Plan & license: the two danger rows exist only with a licence, which is Premium.
    let mut licence = vec![if basic {
        let mut r = row(None, (c::YOUR_PLAN, c::PLAN_BASIC_DESC), Kind::Status, json!(c::PLAN_BASIC));
        r.button = Some(c::SEE_PLANS);
        r
    } else {
        row(None, (c::YOUR_PLAN, c::PLAN_PREMIUM.1), Kind::Status, json!(c::PLAN_PREMIUM.0))
    }];
    if !basic {
        for (l, armed, confirm) in [(c::DEACTIVATE, c::DEACTIVATE_ARMED, c::DEACTIVATE_CONFIRM), (c::MOVE, c::MOVE_ARMED, c::MOVE_CONFIRM)]
        {
            let mut r = row(None, l, Kind::Danger, Value::Null);
            r.armed = Some((armed, confirm));
            licence.push(r);
        }
    }

    let mut shortcuts = row(None, c::SHORTCUTS, Kind::Status, Value::Null);
    shortcuts.button = Some(c::SHOW_SHORTCUTS);
    let mut report = row(None, c::REPORT, Kind::Status, Value::Null);
    report.button = Some(c::GET_SUPPORT);

    vec![
        Section { title: "", hint: None, rows: vec![destination] },
        Section { title: c::DEFAULTS, hint: Some(c::DEFAULTS_HINT), rows: defaults },
        Section { title: c::MORE, hint: Some(c::MORE_HINT), rows: more },
        Section { title: c::APPEARANCE, hint: None, rows: appearance },
        Section { title: c::SCHEDULE, hint: Some(if basic { c::SCHEDULE_HINT_LOCKED } else { c::SCHEDULE_HINT }), rows: schedule },
        Section { title: c::SOFTWARE, hint: Some(c::SOFTWARE_HINT), rows: software },
        Section { title: c::ADVANCED, hint: Some(c::ADVANCED_HINT), rows: advanced },
        Section { title: c::PLAN, hint: None, rows: licence },
        Section { title: c::HELP, hint: None, rows: vec![shortcuts, report] },
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn keys(secs: &[Section]) -> BTreeSet<&'static str> {
        secs.iter().flat_map(|s| &s.rows).filter_map(|r| r.key).collect()
    }

    fn basic() -> State {
        let mut s = State::sample(0);
        s.plan = Plan::Basic;
        s
    }

    #[test]
    fn keys_are_the_config_keys_plus_the_rust_only_rows() {
        // Config keys the Settings tab never shows (05-tabs.md 4.7 lists none of them).
        let not_in_tab = ["schemaVersion", "sponsorBlockCategories", "writeInfoJson", "writeDescription"];
        // Rows that are not config.json keys: Layout and Needs review.
        let rust_only = ["layout", "holdUnconfirmed"];
        let config = ahg_engine::config::default_config();
        let mut expected: BTreeSet<&str> = config.keys().map(String::as_str).filter(|k| !not_in_tab.contains(k)).collect();
        expected.extend(rust_only);
        assert!(expected.len() > 30);
        let secs = sections(&State::sample(0));
        let got = keys(&secs);
        assert_eq!(got, expected);
        let total = secs.iter().flat_map(|s| &s.rows).filter(|r| r.key.is_some()).count();
        assert_eq!(total, got.len(), "a key appears on two rows");
        assert_eq!(secs.len(), 9);
    }

    #[test]
    fn on_basic_every_gated_row_carries_its_lock() {
        // What the engine's tier module clamps, probed rather than listed.
        let engine = [
            ("defaultFormat", tier::clamp_format("flac", Plan::Basic) != "flac"),
            ("defaultQuality", tier::clamp_audio_quality("lossless", Plan::Basic) != "lossless"),
            ("defaultVideoQuality", tier::clamp_video_quality(VideoQuality::P2160, Plan::Basic) != VideoQuality::P2160),
            ("maxConcurrentDownloads", tier::clamp_concurrency(5.0, Plan::Basic) < 5),
            ("scheduleEnabled", !BASIC_LIMITS.scheduler),
            ("subtitleLangs", BASIC_LIMITS.max_subtitle_langs < 2),
            ("subtitleAuto", !BASIC_LIMITS.subtitle_auto),
            ("splitChapters", !BASIC_LIMITS.split_chapters),
            ("cookieBrowser", !BASIC_LIMITS.cookies),
        ];
        let expected: BTreeSet<&str> = engine.iter().filter(|(_, locked)| *locked).map(|(k, _)| *k).collect();
        assert_eq!(expected.len(), 9);
        let secs = sections(&basic());
        let gated: BTreeSet<&str> = secs.iter().flat_map(|s| &s.rows).filter(|r| r.gated()).filter_map(|r| r.key).collect();
        assert_eq!(gated, expected);
        for r in secs.iter().flat_map(|s| &s.rows).filter(|r| r.gated()) {
            let sentence = match &r.kind {
                Kind::Select(o) => o.iter().find_map(|o| o.lock.clone()),
                _ => r.lock.clone(),
            };
            assert!(sentence.is_some_and(|t| t.ends_with("a Premium feature.")), "{:?}", r.key);
        }
        let all = sections(&State::sample(0));
        assert!(all.iter().flat_map(|s| &s.rows).all(|r| !r.gated()), "Premium locks nothing");
    }

    #[test]
    fn locked_choices_are_exactly_the_premium_ones() {
        let secs = sections(&basic());
        let locked = |key: &str| -> Vec<String> {
            let row = secs.iter().flat_map(|s| &s.rows).find(|r| r.key == Some(key)).unwrap();
            let Kind::Select(options) = &row.kind else { panic!("{key} is not a select") };
            options.iter().filter(|o| o.lock.is_some()).map(|o| o.value.clone()).collect()
        };
        assert_eq!(locked("defaultFormat"), ["flac", "wav", "alac"]);
        assert_eq!(locked("defaultQuality"), ["lossless"]);
        assert_eq!(locked("defaultVideoQuality"), ["2160p", "best"]);
        assert_eq!(locked("maxConcurrentDownloads"), ["3", "4", "5"]);
        assert_eq!(locked("cookieBrowser").len(), 7);
    }

    #[test]
    fn dimmed_follows_the_switches() {
        let mut s = State::sample(0);
        let dim = |s: &State, key: &str| sections(s).into_iter().flat_map(|x| x.rows).find(|r| r.key == Some(key)).unwrap().dimmed;
        assert!(dim(&s, "subtitleAuto") && dim(&s, "scheduleTime"), "off by default");
        s.settings.insert("subtitleMode".into(), json!("embed"));
        s.settings.insert("scheduleEnabled".into(), json!(true));
        assert!(!dim(&s, "subtitleAuto") && !dim(&s, "scheduleDays"));
        s.plan = Plan::Basic;
        assert!(dim(&s, "scheduleTime"), "the scheduler stays off on Basic");
    }
}
