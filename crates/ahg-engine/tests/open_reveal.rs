//! Which paths the app may show in their folder or open, as the
//! `shell:showItemInFolder` and `shell:openItem` handlers of electron/ipc.ts
//! decide (ipc.test.ts :521-:555). Opening RUNS what it is given, so only the
//! kinds of file the app itself produces, inside the download folder, pass.
use ahg_engine::validate::{openable, revealable};

const ROOT: &str = "C:\\Downloads";

#[test]
fn a_file_inside_the_download_folder_may_be_shown() {
    assert_eq!(revealable("C:\\Downloads\\song.mp3", ROOT), Ok(Some("C:\\Downloads\\song.mp3".to_owned())));
    assert_eq!(revealable("sub\\clip.mp4", ROOT), Ok(Some("C:\\Downloads\\sub\\clip.mp4".to_owned())));
    // Any kind of file may be shown: showing runs nothing.
    assert_eq!(revealable("C:\\Downloads\\notes.txt", ROOT), Ok(Some("C:\\Downloads\\notes.txt".to_owned())));
    // D29: a download folder that is a whole drive.
    assert_eq!(revealable("D:\\song.mp3", "D:\\"), Ok(Some("D:\\song.mp3".to_owned())));
}

#[test]
fn a_path_outside_the_download_folder_is_refused() {
    for outside in ["..\\Windows\\System32\\cmd.exe", "C:\\Windows\\notepad.exe", "D:\\Music\\x.mp3", "\\\\server\\share\\x.mp3", "..\\Downloads-evil\\x.mp3"] {
        assert_eq!(revealable(outside, ROOT), Err("Invalid input: path escapes the output directory".to_owned()), "{outside}");
        assert_eq!(openable(outside, ROOT), Err("Invalid input: not an openable media file in the output directory".to_owned()), "{outside}");
    }
}

#[test]
fn an_empty_path_is_not_an_error_just_nothing_to_show() {
    for blank in ["", "   ", "\t\n"] {
        assert_eq!(revealable(blank, ROOT), Ok(None));
        assert_eq!(openable(blank, ROOT), Ok(None));
    }
}

#[test]
fn only_media_the_app_produces_may_be_opened() {
    for media in ["song.mp3", "song.M4A", "a.flac", "a.opus", "a.wav", "clip.mp4", "clip.MKV", "clip.webm", "sub\\a.ogg"] {
        assert!(matches!(openable(media, ROOT), Ok(Some(_))), "{media}");
    }
    for other in ["setup.exe", "run.bat", "a.mp3.exe", "script.ps1", "page.html", "mp3", "noextension", "a.mp3 ", "a.lnk", "C:\\Downloads"] {
        assert_eq!(openable(other, ROOT), Err("Invalid input: not an openable media file in the output directory".to_owned()), "{other}");
    }
}

#[test]
fn an_absurdly_long_path_is_refused_outright() {
    let long = format!("C:\\Downloads\\{}.mp3", "a".repeat(5000));
    assert_eq!(revealable(&long, ROOT), Err("Invalid input: expected a string within length limits".to_owned()));
    assert_eq!(openable(&long, ROOT), Err("Invalid input: expected a string within length limits".to_owned()));
}
