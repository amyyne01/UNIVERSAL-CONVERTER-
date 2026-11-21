//! The scheduled PC shutdown (step 69). A stub until then.
pub struct Power;

impl super::Part for Power {
    fn name(&self) -> &'static str {
        "power"
    }
}
