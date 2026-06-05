use alloc::string::String;
use core::sync::atomic::Ordering;
use crate::{println, print, serial_println};
use crate::fs::{read_file, write_file, create_dir, list_dir};
use crate::reset_color;
use super::Command;
use crate::shell::flags::Flags;

pub struct WriteCommand;
impl Command for WriteCommand {
    fn name(&self) -> &'static str { "write" }
    fn description(&self) -> &'static str { "Write a file: write <filename> <contents>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: write <filename> <contents>"); return; }
        let contents = args[1..].join(" ");
        if write_file(&args[0], contents.as_bytes()) {
            println!("Wrote {} bytes to {}", contents.len(), args[0]);
        } else {
            println!("Failed to write {}", args[0]);
        }
    }
}

pub struct ReadCommand;
impl Command for ReadCommand {
    fn name(&self) -> &'static str { "read" }
    fn description(&self) -> &'static str { "Read a file: read <filename>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: read <filename>"); return; }
        match read_file(&args[0]) {
            Some(data) => println!("{}", core::str::from_utf8(&data).unwrap_or("(not utf8)")),
            None => println!("Failed to read {}", args[0]),
        }
    }
}

pub struct LsCommand;
impl Command for LsCommand {
    fn name(&self) -> &'static str { "ls" }
    fn description(&self) -> &'static str { "List directory contents" }
    fn execute(&self, args: &[String]) {
        let flags = Flags::parse(args);
        let path = flags.first().unwrap_or("");
        let long = flags.has('l');
        let all = flags.has('a');

        for entry in crate::fs::list_dir(path) {
            // Skip dot entries unless -a is set
            if !all && (entry.name == "." || entry.name == "..") {
                continue;
            }

            if long {
                let (yr, mo, day, hr, min, _sec) = entry.modified;
                if entry.is_dir {
                    println!("\x1b[32mDIR   {:<20}       {}-{:02}-{:02} {:02}:{:02}",
                             entry.name, yr, mo, day, hr, min);
                    reset_color!();
                } else {
                    println!("FILE  {:<20} {:>6}B  {}-{:02}-{:02} {:02}:{:02}",
                             entry.name, entry.size, yr, mo, day, hr, min);
                }
            } else {
                if entry.is_dir {
                    print!("\x1b[32m{} ", entry.name);
                } else {
                    print!("{} ", entry.name);
                }
            }
        }

        if !long {
            reset_color!();
            println!();
        }
    }
}

pub struct MkdirCommand;
impl Command for MkdirCommand {
    fn name(&self) -> &'static str { "mkdir" }
    fn description(&self) -> &'static str { "Create a directory: mkdir <dirname>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: mkdir <dirname>"); return; }
        if !create_dir(&args[0]) {
            println!("Failed to create directory");
        }
    }
}

pub struct EditCommand;
impl Command for EditCommand {
    fn name(&self) -> &'static str { "edit" }
    fn description(&self) -> &'static str { "Open a file in the editor: edit <filename>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: edit <filename>"); return; }
        let filename = args[0].clone();
        // Suppress the shell prompt that would otherwise print after this command
        crate::task::executor::SUPPRESS_PROMPT.store(true, Ordering::SeqCst);
        crate::task::executor::spawn_task(
            crate::task::Task::new(async move {
                crate::program::editor::Editor::run(&filename).await;
            })
        );
    }
}

pub struct DeleteCommand;
impl Command for DeleteCommand {
    fn name(&self) -> &'static str { "rm" }
    fn description(&self) -> &'static str { "Delete a file or directory: rm <path>" }
    fn execute(&self, args: &[String]) {
        let flags = Flags::parse(args);
        let path = match flags.first() {
            Some(p) => p,
            None => { println!("Usage: rm <path>"); return; }
        };
        if !crate::fs::delete_file(path) {
            println!("Failed to delete {}", path);
        }
    }
}

pub struct CatCommand;
impl Command for CatCommand {
    fn name(&self) -> &'static str { "cat" }
    fn description(&self) -> &'static str { "Print file contents: cat <filename>" }
    fn execute(&self, args: &[String]) {
        ReadCommand.execute(args);
    }
}

pub struct CpCommand;
impl Command for CpCommand {
    fn name(&self) -> &'static str { "cp" }
    fn description(&self) -> &'static str { "Copy a file: cp <src> <dst>" }
    fn execute(&self, args: &[String]) {
        let flags = Flags::parse(args);
        if flags.args.len() < 2 { println!("Usage: cp <src> <dst>"); return; }
        if crate::fs::copy_file(&flags.args[0], &flags.args[1]) {
            println!("Copied {} -> {}", flags.args[0], flags.args[1]);
        } else {
            println!("cp: failed to copy {} to {}", flags.args[0], flags.args[1]);
        }
    }
}

pub struct MvCommand;
impl Command for MvCommand {
    fn name(&self) -> &'static str { "mv" }
    fn description(&self) -> &'static str { "Move or rename a file: mv <src> <dst>" }
    fn execute(&self, args: &[String]) {
        let flags = Flags::parse(args);
        if flags.args.len() < 2 { println!("Usage: mv <src> <dst>"); return; }
        if crate::fs::move_file(&flags.args[0], &flags.args[1]) {
            println!("Moved {} -> {}", flags.args[0], flags.args[1]);
        } else {
            println!("mv: failed to move {} to {}", flags.args[0], flags.args[1]);
        }
    }
}

pub struct PwdCommand;
impl Command for PwdCommand {
    fn name(&self) -> &'static str { "pwd" }
    fn description(&self) -> &'static str { "Print working directory" }
    fn execute(&self, _args: &[String]) {
        println!("{}", crate::fs::get_current_dir());
    }
}

pub struct CdCommand;
impl Command for CdCommand {
    fn name(&self) -> &'static str { "cd" }
    fn description(&self) -> &'static str { "Change directory: cd <path>" }
    fn execute(&self, args: &[String]) {
        let path = args.first().map(|s| s.as_str()).unwrap_or("/");
        if !crate::fs::set_current_dir(path) {
            println!("cd: {}: No such directory", path);
        }
    }
}

pub struct TouchCommand;
impl Command for TouchCommand {
    fn name(&self) -> &'static str { "touch" }
    fn description(&self) -> &'static str { "Create empty file: touch <filename>" }
    fn execute(&self, args: &[String]) {
        if args.is_empty() { println!("Usage: touch <filename>"); return; }
        if !crate::fs::write_file(&args[0], b"") {
            println!("touch: failed to create {}", args[0]);
        }
    }
}