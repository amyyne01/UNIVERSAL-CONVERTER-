//! Handing a file to Windows: showing it in its folder, or opening it in the
//! program the user has chosen for that kind of file. Which paths may be
//! shown or opened is decided before this is reached (`validate::revealable`,
//! `validate::openable`); nothing here is given a path from outside unchecked.

/// The desktop the app runs on. The real one is `Windows`; tests record calls.
pub trait Desktop: Send + Sync {
    /// Open the file's folder with the file selected.
    fn reveal(&self, path: &str);
    /// Open the file in its default program. False when Windows could not.
    fn open(&self, path: &str) -> bool;
}

pub struct Windows;

#[cfg(windows)]
impl Desktop for Windows {
    fn reveal(&self, path: &str) {
        use std::os::windows::process::CommandExt;
        // Explorer reads its own command line: the path goes in quotes, as
        // written. A Windows path cannot hold a quote, so nothing can end them early.
        let _ = std::process::Command::new("explorer.exe").raw_arg(format!("/select,\"{path}\"")).spawn();
    }

    fn open(&self, path: &str) -> bool {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let (verb, file) = (wide("open"), wide(path));
        // No command line is built: Windows is given the path itself.
        let result = unsafe { ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), std::ptr::null(), std::ptr::null(), SW_SHOWNORMAL) };
        result as isize > 32
    }
}

#[cfg(not(windows))]
impl Desktop for Windows {
    fn reveal(&self, _: &str) {}

    fn open(&self, _: &str) -> bool {
        false
    }
}
