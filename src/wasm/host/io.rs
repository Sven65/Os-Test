use wasmi::{Caller, Linker, Extern};
use crate::wasm::state::HostState;

pub fn register(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    linker.func_wrap("os", "print", |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
        let Some(Extern::Memory(mem)) = caller.get_export("memory") else { return; };
        let data = mem.data(&caller);
        let start = ptr as usize;
        let end = start.saturating_add(len as usize);
        if let Some(bytes) = data.get(start..end) {
            crate::print!("{}", core::str::from_utf8(bytes).unwrap_or("<invalid utf8>"));
        }
    })?;
    Ok(())
}