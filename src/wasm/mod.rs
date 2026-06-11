use alloc::vec::Vec;
use wasmi::{Engine, Linker, Module, Store};
use crate::wasm::state::HostState;

pub mod state;
mod host;


pub fn run(data: Vec<u8>) -> Result<(), wasmi::Error> {
    let engine = Engine::default();
    let module = Module::new(&engine, data)?;
    let mut store = Store::new(&engine, HostState::default());
    let mut linker = Linker::new(&engine);
    host::register_all(&mut linker)?;

    let instance = linker.instantiate_and_start(&mut store, &module)?;
    instance.get_typed_func::<(), ()>(&store, "main")?.call(&mut store, ())
}