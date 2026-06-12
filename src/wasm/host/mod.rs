pub mod io;
pub mod proc;

use wasmi::Linker;
use crate::wasm::state::HostState;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// A group of host functions under one wasm import module (e.g. "os::io").
pub trait HostModule {
    /// The wasm import module name, e.g. "os::io"
    fn namespace(&self) -> &'static str;
    /// Register all functions in this module on the linker.
    fn register(&self, linker: &mut Linker<HostState>) -> Result<(), wasmi::Error>;
}

fn modules() -> Vec<Box<dyn HostModule>> {
    alloc::vec![
        Box::new(io::IoModule),
        //Box::new(proc::ProcModule),
        // Box::new(fs::FsModule),  // sen
    ]
}

pub fn register_all(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    for m in modules() {
        m.register(linker)?;
    }
    Ok(())
}