use alloc::vec::Vec;
use alloc::string::{String, ToString};
use alloc::collections::BTreeMap;

pub struct Flags {
    pub short: Vec<char>,
    pub long: Vec<String>,
    pub values: BTreeMap<String, String>, // -o val or --output=val
    pub args: Vec<String>,               // positional args
}

impl Flags {
    pub fn parse(args: &[String]) -> Self {
        let mut short = Vec::new();
        let mut long = Vec::new();
        let mut values = BTreeMap::new();
        let mut positional = Vec::new();
        let mut end_of_flags = false;
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            if end_of_flags {
                positional.push(arg.clone());
            } else if arg == "--" {
                end_of_flags = true;
            } else if arg.starts_with("--") {
                // Long flag: --foo or --foo=bar
                let flag = &arg[2..];
                if let Some(eq) = flag.find('=') {
                    let key = flag[..eq].to_string();
                    let val = flag[eq+1..].to_string();
                    values.insert(key.clone(), val);
                    long.push(key);
                } else if i + 1 < args.len() && !args[i+1].starts_with('-') {
                    // --foo bar
                    values.insert(flag.to_string(), args[i+1].clone());
                    long.push(flag.to_string());
                    i += 1;
                } else {
                    long.push(flag.to_string());
                }
            } else if arg.starts_with('-') && arg.len() > 1 {
                // Short flags: -l or -la or -o value
                let chars: Vec<char> = arg[1..].chars().collect();
                if chars.len() == 1 {
                    // Could be -o value
                    let ch = chars[0];
                    if i + 1 < args.len() && !args[i+1].starts_with('-') {
                        // Peek ahead — only treat as value if next arg doesn't look like a flag
                        // For now just add as flag, caller can use get_value if needed
                        short.push(ch);
                    } else {
                        short.push(ch);
                    }
                } else {
                    // -la style, all are boolean flags
                    for ch in chars {
                        short.push(ch);
                    }
                }
            } else {
                positional.push(arg.clone());
            }

            i += 1;
        }

        Flags { short, long, values, args: positional }
    }

    /// Check for a short flag: -l
    pub fn has(&self, flag: char) -> bool {
        self.short.contains(&flag)
    }

    /// Check for a long flag: --verbose
    pub fn has_long(&self, flag: &str) -> bool {
        self.long.iter().any(|f| f == flag)
    }

    /// Get value for a flag: --output=foo or --output foo
    pub fn value(&self, flag: &str) -> Option<&str> {
        self.values.get(flag).map(|s| s.as_str())
    }

    /// First positional arg
    pub fn first(&self) -> Option<&str> {
        self.args.first().map(|s| s.as_str())
    }

    /// Nth positional arg
    pub fn get(&self, index: usize) -> Option<&str> {
        self.args.get(index).map(|s| s.as_str())
    }
}