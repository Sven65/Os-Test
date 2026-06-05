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

pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: (u16, u16, u16, u16, u16, u16), // year, month, day, hour, min, sec
}

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

type Fs = FileSystem<VirtioBlockDevice, RtcTimeProvider>;

pub static FS: Mutex<Option<Fs>> = Mutex::new(None);

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

pub fn read_file(path: &str) -> Option<Vec<u8>> {
    let mut guard = FS.lock();
    let fs = guard.as_mut()?;
    let root = fs.root_dir();
    let mut file = root.open_file(path).ok()?;
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
    let mut guard = FS.lock();
    let fs = guard.as_mut().unwrap();
    let root = fs.root_dir();
    let mut file = match root.create_file(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    file.write_all(data).is_ok()
}

pub fn append_file(path: &str, data: &[u8]) -> bool {
    let mut guard = FS.lock();
    let fs = guard.as_mut().unwrap();
    let root = fs.root_dir();
    let mut file = match root.open_file(path) {
        Ok(f) => f,
        Err(_) => match root.create_file(path) {
            Ok(f) => f,
            Err(_) => return false,
        }
    };
    file.seek(fatfs::SeekFrom::End(0)).ok();
    file.write_all(data).is_ok()
}

pub fn format_disk() -> bool {
    serial_println!("[fs] format_disk: not supported while mounted");
    false
}

pub fn create_dir(path: &str) -> bool {
    let mut guard = FS.lock();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return false,
    };
    let root = fs.root_dir();
    let result = root.create_dir(path).is_ok();
    result
}

pub fn list_dir(path: &str) -> Vec<DirEntry> {
    let mut guard = FS.lock();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return Vec::new(),
    };
    let root = fs.root_dir();
    let dir = if path.is_empty() || path == "/" {
        root
    } else {
        match root.open_dir(path) {
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

pub fn delete_file(path: &str) -> bool {
    let mut guard = FS.lock();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return false,
    };
    let root = fs.root_dir();
    let result = root.remove(path).is_ok();
    result
}

pub fn delete_dir(path: &str) -> bool {
    let mut guard = FS.lock();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return false,
    };
    let root = fs.root_dir();
    let result = root.remove(path).is_ok();
    result
}