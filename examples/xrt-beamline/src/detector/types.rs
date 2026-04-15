/// Tracks whether any parameter has changed since last checked.
#[derive(Default)]
pub struct DirtyFlags {
    pub any: bool,
}

impl DirtyFlags {
    pub fn set(&mut self) {
        self.any = true;
    }

    /// Atomically take and clear dirty flags.
    pub fn take(&mut self) -> DirtyFlags {
        let flags = DirtyFlags { any: self.any };
        self.any = false;
        flags
    }
}
