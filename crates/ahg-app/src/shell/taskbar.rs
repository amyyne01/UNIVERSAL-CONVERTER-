//! The taskbar's percent bar, sampled once a second (step 65). A stub until then.
pub struct Taskbar;

impl super::Part for Taskbar {
    fn name(&self) -> &'static str {
        "taskbar"
    }
}
