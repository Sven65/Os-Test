pub mod virtio_fs;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use fatfs::{FileSystem, FsOptions, FormatVolumeOptions, Read, Write, Seek, TimeProvider, Date, Time, DateTime};
use spin::Mutex;
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::pci::PciTransport;
use crate::device::virtio_hal::OsHal;
use crate::serial_println;
use virtio_fs::VirtioBlockDevice;

#[derive(Debug)]
pub struct RtcTimeProvider;

impl TimeProvider for RtcTimeProvider {
    fn get_current_date(&self) -> Date {
        let t = crate::time::get_time();
        Date::new(t.year as u16, t.month as u16, t.day as u16)
    }

    fn get_current_date_time(&self) -> DateTime {
        let t = crate::time::get_time();
        DateTime::new(
            Date::new(t.year as u16, t.month as u16, t.day as u16),
            Time::new(t.hour as u16, t.minute as u16, t.second as u16, 0),
        )
    }
}

pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: (u16, u16, u16, u16, u16, u16),
}

type Fs = FileSystem<VirtioBlockDevice, RtcTimeProvider>;

pub static FS: Mutex<Option<Fs>> = Mutex::new(None);

lazy_static::lazy_static! {
    static ref CURRENT_DIR: Mutex<String> = Mutex::new(String::from("/"));
}

pub fn init(blk: VirtIOBlk<OsHal, PciTransport>) {
    let mut dev = VirtioBlockDevice::new(blk);

    let mut buf = [0u8; 512];
    dev.read(&mut buf).expect("failed to read sector 0");
    dev.seek(fatfs::SeekFrom::Start(0)).expect("seek failed");

    let sig = u16::from_le_bytes([buf[510], buf[511]]);

    if sig != 0xAA55 {
        serial_println!("[fs] No filesystem found, formatting...");
        fatfs::format_volume(&mut dev, FormatVolumeOptions::new())
            .expect("format failed");
        serial_println!("[fs] Formatted.");
    } else {
        serial_println!("[fs] Filesystem found, mounting.");
    }

    let fs = FileSystem::new(dev, FsOptions::new().time_provider(RtcTimeProvider))
        .expect("mount failed");
    *FS.lock() = Some(fs);
    serial_println!("[fs] Mounted.");
}

pub fn resolve_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        let current = CURRENT_DIR.lock().clone();
        if current == "/" {
            path.to_string()
        } else {
            alloc::format!("{}/{}", current, path)
        }
    }
}

/// Split a resolved path into (parent_dir, filename)
/// e.g. "balls/test" -> ("balls", "test")
///      "test" -> ("", "test")
///      "/balls/test" -> ("/balls", "test")
fn split_path(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(0) => ("/", &path[1..]),
        Some(pos) => (&path[..pos], &path[pos+1..]),
        None => ("", path),
    }
}

/// Open the parent directory of a path, or root if no parent
macro_rules! open_parent {
    ($root:expr, $dir:expr) => {
        if $dir.is_empty() || $dir == "/" {
            $root
        } else {
            match $root.open_dir($dir) {
                Ok(d) => d,
                Err(_) => return false,
            }
        }
    }
}

pub fn get_current_dir() -> String {
    let dir = CURRENT_DIR.lock().clone();
    if dir.is_empty() { String::from("/") } else { dir }
}

pub fn set_current_dir(path: &str) -> bool {
    let new_path = if path == ".." {
        let current = CURRENT_DIR.lock().clone();
        if current == "/" || current.is_empty() {
            String::from("/")
        } else {
            match current.rfind('/') {
                Some(0) | None => String::from("/"),
                Some(pos) => current[..pos].to_string(),
            }
        }
    } else if path == "/" {
        String::from("/")
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        resolve_path(path)
    };

    if new_path == "/" {
        *CURRENT_DIR.lock() = String::from("/");
        return true;
    }

    let exists = {
        let mut guard = FS.lock();
        let fs = match guard.as_mut() {
            Some(fs) => fs,
            None => return false,
        };
        let root = fs.root_dir();
        let result = root.open_dir(&new_path).is_ok();
        result
    };

    if exists {
        *CURRENT_DIR.lock() = new_path;
    }
    exists
}

pub fn read_file(path: &str) -> Option<Vec<u8>> {
    let path = resolve_path(path);
    let (dir, filename) = split_path(&path);
    let mut guard = FS.lock();
    let fs = guard.as_mut()?;
    let root = fs.root_dir();
    let parent = if dir.is_empty() || dir == "/" {
        root
    } else {
        root.open_dir(dir).ok()?
    };
    let mut file = parent.open_file(filename).ok()?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => return None,
        }
    }
    Some(buf)
}

pub fn write_file(path: &str, data: &[u8]) -> bool {
    let path = resolve_path(path);
    let (dir, filename) = split_path(&path);
    let mut guard = FS.lock();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return false,
    };
    let root = fs.root_dir();
    let parent = open_parent!(root, dir);
    let mut file = match parent.create_file(filename) {
        Ok(f) => f,
        Err(_) => return false,
    };
    if file.truncate().is_err() {
        return false;
    }
    file.write_all(data).is_ok()
}

pub fn append_file(path: &str, data: &[u8]) -> bool {
    let path = resolve_path(path);
    let (dir, filename) = split_path(&path);
    let mut guard = FS.lock();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return false,
    };
    let root = fs.root_dir();
    let parent = open_parent!(root, dir);
    let mut file = match parent.open_file(filename) {
        Ok(f) => f,
        Err(_) => match parent.create_file(filename) {
            Ok(f) => f,
            Err(_) => return false,
        }
    };
    file.seek(fatfs::SeekFrom::End(0)).ok();
    file.write_all(data).is_ok()
}

pub fn delete_file(path: &str) -> bool {
    let path = resolve_path(path);
    let (dir, filename) = split_path(&path);
    let mut guard = FS.lock();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return false,
    };
    let root = fs.root_dir();
    let parent = open_parent!(root, dir);
    parent.remove(filename).is_ok()
}

pub fn delete_dir(path: &str) -> bool {
    delete_file(path)
}

pub fn create_dir(path: &str) -> bool {
    let path = resolve_path(path);
    let (dir, dirname) = split_path(&path);
    let mut guard = FS.lock();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return false,
    };
    let root = fs.root_dir();
    let parent = open_parent!(root, dir);
    let result = parent.create_dir(dirname).is_ok();
    result
}

pub fn copy_file(src: &str, dst: &str) -> bool {
    let data = match read_file(src) {
        Some(d) => d,
        None => return false,
    };
    write_file(dst, &data)
}

pub fn move_file(src: &str, dst: &str) -> bool {
    if !copy_file(src, dst) {
        return false;
    }
    delete_file(src)
}

pub fn format_disk() -> bool {
    serial_println!("[fs] format_disk: not supported while mounted");
    false
}

pub fn list_dir(path: &str) -> Vec<DirEntry> {
    let path = if path.is_empty() {
        CURRENT_DIR.lock().clone()
    } else {
        resolve_path(path)
    };
    let mut guard = FS.lock();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return Vec::new(),
    };
    let root = fs.root_dir();
    let dir = if path == "/" || path.is_empty() {
        root
    } else {
        match root.open_dir(&path) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        }
    };
    let mut entries = Vec::new();
    for entry in dir.iter() {
        if let Ok(e) = entry {
            let name = core::str::from_utf8(e.short_file_name_as_bytes())
                .unwrap_or("?")
                .to_string();
            let m = e.modified();
            entries.push(DirEntry {
                name,
                is_dir: e.is_dir(),
                size: e.len(),
                modified: (
                    m.date.year,
                    m.date.month,
                    m.date.day,
                    m.time.hour,
                    m.time.min,
                    m.time.sec,
                ),
            });
        }
    }
    entries
}