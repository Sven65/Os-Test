use alloc::string::String;
use alloc::vec::Vec;
use wasmi::{Caller, Engine, Linker, Module, Store};
use crate::fs::read_file;
use crate::println;
use crate::shell::commands::Command;

fn run_program(data: Vec<u8>) -> Result<(), wasmi::Error> {
    let engine = Engine::default();
    let module = Module::new(&engine, data)?;

    type HostState = u32;
    let mut store = Store::new(&engine, 42);

    // A linker can be used to instantiate Wasm modules.
    // The job of a linker is to satisfy the Wasm module's imports.
    let mut linker = <Linker<HostState>>::new(&engine);
    // We are required to define all imports before instantiating a Wasm module.
    linker.func_wrap("host", "hello", |caller: Caller<'_, HostState>, param: i32| {
        println!("Got {param} from WebAssembly and my host state is: {}", caller.data());
    })?;
    let instance = linker.instantiate_and_start(&mut store, &module)?;
    // Now we can finally query the exported "hello" function and call it.
    instance
        .get_typed_func::<(), ()>(&store, "hello")?
        .call(&mut store, ())?;
    Ok(())
}

pub struct RunCommand;
impl Command for RunCommand {
    fn name(&self) -> &'static str { "run" }
    fn description(&self) -> &'static str { "Run a program" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: run <filename>"); return; }
        match read_file(&args[0]) {
            Some(data) => {
                // Execute code
                
                match run_program(data) {
                    Ok(()) => println!("Program completed"),
                    Err(e) => println!("Error during program execution: {}", e),
                }
            },
            None => println!("Failed to read {}", args[0]),
        }
    }
}