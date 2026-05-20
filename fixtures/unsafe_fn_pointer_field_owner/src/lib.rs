pub(crate) struct TaskVTable {
    /// Schedules a raw task pointer.
    pub(crate) schedule: unsafe fn(*const ()),
}
