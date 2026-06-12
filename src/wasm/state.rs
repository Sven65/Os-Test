#[derive(Default)]
pub struct HostState {
    pub exit_code: Option<i32>,
    pub args: alloc::vec::Vec<alloc::string::String>,
    // pub files: BTreeMap<i32, FileHandle>,
}
