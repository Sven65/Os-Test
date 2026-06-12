pub mod help;
mod system;
mod fs;
mod misc;
mod net;
mod prog;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

pub trait Command {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn execute(&self, args: &[String]);
}

pub fn get_commands() -> Vec<&'static dyn Command> {
    vec![
        &system::HelpCommand,
        &system::DevicesCommand,
        &system::RaddrCommand,
        &system::AhciCommand,
        &system::DumpCommand,
        &system::ConfigCommand,
        &system::MemInfoCommand,
        &fs::WriteCommand,
        &fs::ReadCommand,
        &fs::LsCommand,
        &fs::MkdirCommand,
        &fs::EditCommand,
        &fs::DeleteCommand,
        &fs::CatCommand,
        &fs::CpCommand,
        &fs::MvCommand,
        &fs::PwdCommand,
        &fs::CdCommand,
        &fs::TouchCommand,
        &misc::ClearCommand,
        &misc::RandCommand,
        &misc::TimeCommand,
        &misc::ColorCommand,
        &misc::BitsCommand,
        &misc::ExitCommand,
        &misc::EchoCommand,
        &net::NetCommand,
        &net::PingCommand,
        &net::FetchCommand,
        &prog::RunCommand,
    ]
}

pub fn find_command(name: &str) -> Option<&'static dyn Command> {
    get_commands().into_iter().find(|c| c.name() == name)
}