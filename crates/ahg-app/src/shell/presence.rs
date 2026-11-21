//! Discord presence (step 64). A stub until then.
pub struct Presence;

impl super::Part for Presence {
    fn name(&self) -> &'static str {
        "presence"
    }
}
