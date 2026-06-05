pub mod virtio_fs;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use fatfs::{FileSystem, FsOptions, FormatVolumeOptions, Read, Write, Seek};
use spin::Mutex;
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::pci::PciTransport;
use crate::device::virtio_hal::OsHal;
use crate::serial_println;
use virtio_fs::VirtioBlockDevice;

pub static FS: Mutex<Option<FileSystem<VirtioBlockDevice>>> = Mutex::new(None);

pub fn init(blk: VirtIOBlk<OsHal, PciTransport>) {
    let mut dev = VirtioBlockDevice::new(blk);

    // Check for FAT signature using the Read trait
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

    let fs = FileSystem::new(dev, FsOptions::new()).expect("mount failed");
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
        Err(_) => {
            // Create if doesn't exist
            match root.create_file(path) {
                Ok(f) => f,
                Err(_) => return false,
            }
        }
    };

    file.seek(fatfs::SeekFrom::End(0)).ok();
    file.write_all(data).is_ok()
}

pub fn format_disk() -> bool {
    // Can't reformat while mounted — would need to reinitialize entirely
    // For now, just signal that format is needed on next init
    serial_println!("[fs] format_disk: not supported while mounted, call init() with a fresh device");
    false
}

pub fn create_dir(path: &str) -> bool {
    let result = {
        let mut guard = FS.lock();
        let fs = match guard.as_mut() {
            Some(fs) => fs,
            None => return false,
        };
        let root = fs.root_dir();
        let r = root.create_dir(path);
        r.is_ok()
    };
    result
}

pub fn list_dir() -> Vec<(String, bool)> {
    let mut guard = FS.lock();
    let fs = match guard.as_mut() {
        Some(fs) => fs,
        None => return Vec::new(),
    };
    let root = fs.root_dir();
    let mut entries = Vec::new();
    for entry in root.iter() {
        if let Ok(e) = entry {
            let name = core::str::from_utf8(e.short_file_name_as_bytes())
                .unwrap_or("?")
                .to_string();
            entries.push((name, e.is_dir()));
        }
    }
    entries
}