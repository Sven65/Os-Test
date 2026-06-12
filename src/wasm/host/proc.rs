use wasmi::{Caller, Error, Extern, Linker};
use crate::wasm::state::HostState;
use super::HostModule;

pub struct ProcModule;

impl HostModule for ProcModule {
    fn namespace(&self) -> &'static str { "os::proc" }

    fn register(&self, linker: &mut Linker<HostState>) -> Result<(), Error> {
        let ns = self.namespace();

        linker.func_wrap(ns, "args_len", |caller: Caller<'_, HostState>| -> i32 {
            let args = &caller.data().args;
            if args.is_empty() { return 0; }
            (args.iter().map(|a| a.len()).sum::<usize>() + args.len() - 1) as i32
        })?;

        linker.func_wrap(ns, "args_get", |mut caller: Caller<'_, HostState>, ptr: i32, max_len: i32| -> i32 {
            let joined = caller.data().args.join("\n");
            let bytes = joined.as_bytes();
            let n = bytes.len().min(max_len.max(0) as usize);
            let Some(Extern::Memory(mem)) = caller.get_export("memory") else { return -2; };
            match mem.data_mut(&mut caller).get_mut(ptr as usize..ptr as usize + n) {
                Some(dst) => { dst.copy_from_slice(&bytes[..n]); n as i32 }
                None => -2,
            }
        })?;

        Ok(())
    }
}