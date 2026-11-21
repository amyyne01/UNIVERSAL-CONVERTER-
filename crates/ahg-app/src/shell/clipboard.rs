//! The clipboard watcher: it offers a link and never downloads (step 62). A stub until then.
pub struct Clipboard;

impl super::Part for Clipboard {
    fn name(&self) -> &'static str {
        "clipboard"
    }
}
