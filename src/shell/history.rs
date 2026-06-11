use alloc::vec::Vec;

const HISTORY_FILE: &str = ".history";
const HISTORY_MAX: usize = 100;

pub struct History {
    entries: Vec<Vec<u8>>,
    index: Option<usize>,
}

impl History {
    pub fn load() -> Self {
        let entries = crate::fs::read_file(HISTORY_FILE)
            .map(|data| {
                data.split(|&b| b == b'\n')
                    .filter(|line| !line.is_empty())
                    .map(|line| line.to_vec())
                    .collect()
            })
            .unwrap_or_default();
        History { entries, index: None }
    }

    /// Called on Enter. Dedupes against last entry, caps size, persists.
    pub fn push(&mut self, cmd: &[u8]) {
        self.index = None;
        if cmd.is_empty() || self.entries.last().map(|e| e.as_slice()) == Some(cmd) {
            return;
        }
        self.entries.push(cmd.to_vec());
        if self.entries.len() > HISTORY_MAX {
            self.entries.remove(0);
        }
        self.save();
    }

    /// Arrow up. Returns the line to display, if any.
    pub fn prev(&mut self) -> Option<Vec<u8>> {
        let next = match self.index {
            None if !self.entries.is_empty() => Some(self.entries.len() - 1),
            Some(i) if i > 0 => Some(i - 1),
            other => other,
        }?;
        self.index = Some(next);
        Some(self.entries[next].clone())
    }

    /// Arrow down. Some(line) = display it (empty = clear line), None = do nothing.
    pub fn next(&mut self) -> Option<Vec<u8>> {
        match self.index {
            Some(i) if i + 1 < self.entries.len() => {
                self.index = Some(i + 1);
                Some(self.entries[i + 1].clone())
            }
            Some(_) => {
                self.index = None;
                Some(Vec::new())
            }
            None => None,
        }
    }

    /// Called on ^C — resets browsing position.
    pub fn reset_cursor(&mut self) {
        self.index = None;
    }

    fn save(&self) {
        let mut data = Vec::new();
        for entry in &self.entries {
            data.extend_from_slice(entry);
            data.push(b'\n');
        }
        crate::fs::write_file(HISTORY_FILE, &data);
    }
}