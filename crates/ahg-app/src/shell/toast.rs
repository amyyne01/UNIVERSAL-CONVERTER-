//! The completion toast through `rune-notify`, silent (step 63, D40). A stub until then.
pub struct Toast;

impl super::Part for Toast {
    fn name(&self) -> &'static str {
        "toast"
    }
}
