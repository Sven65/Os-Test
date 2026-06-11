use wasmi::Linker;
use crate::wasm::state::HostState;

mod time;
mod io;

pub fn register_all(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    io::register(linker)?;
    ///time::register(linker)?;
    Ok(())
}