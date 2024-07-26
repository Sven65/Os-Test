use fatfs::{Error as FatError, FileSystem, FormatVolumeOptions, FsOptions, IoBase, Read, Seek, SeekFrom, Write};
use lazy_static::lazy_static;
use spin::Mutex;

use crate::serial_println;


const RAM_SIZE: usize = 128 * 1024 as usize; // 256KiB RAM for the filesystem

// In-memory block device structure
pub struct RamStorage {
    memory: [u8; RAM_SIZE],
    position: usize, // Current position for reading/writing
}

impl RamStorage {
    pub fn new() -> Self {
        RamStorage {
            memory: [0; RAM_SIZE],
            position: 0,
        }
    }

    pub fn read(&self, offset: u64, buf: &mut [u8]) -> Result<(), ()> {
        let offset = offset as usize;
        if offset + buf.len() > RAM_SIZE {
            return Err(());
        }
        buf.copy_from_slice(&self.memory[offset..offset + buf.len()]);
        Ok(())
    }

    pub fn write(&mut self, offset: u64, buf: &[u8]) -> Result<(), ()> {
        let offset = offset as usize;
        if offset + buf.len() > RAM_SIZE {
            return Err(());
        }
        self.memory[offset..offset + buf.len()].copy_from_slice(buf);
        Ok(())
    }

    pub fn erase(&mut self, offset: u64, len: u64) -> Result<(), ()> {
        let offset = offset as usize;
        let len = len as usize;
        if offset + len > RAM_SIZE {
            return Err(());
        }
        self.memory[offset..offset + len].fill(0);
        Ok(())
    }
}

impl IoBase for RamStorage {
    type Error = FatError<()>;
}


impl Read for RamStorage {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let end = (self.position + buf.len()).min(RAM_SIZE);
        let len = end - self.position;
        buf[..len].copy_from_slice(&self.memory[self.position..self.position + len]);
        self.position = end;
        Ok(len)
    }
}

impl Write for RamStorage {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let end = (self.position + buf.len()).min(RAM_SIZE);
        let len = end - self.position;
        self.memory[self.position..self.position + len].copy_from_slice(&buf[..len]);
        self.position = end;
        Ok(len)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl IoBase for &mut RamStorage {
    type Error = FatError<()>;
}

impl Seek for &mut RamStorage {
    fn seek(&mut self, pos: fatfs::SeekFrom) -> Result<u64, Self::Error> {
        let new_position = match pos {
            SeekFrom::Start(offset) => offset as usize,
            SeekFrom::End(offset) => (RAM_SIZE as i64 + offset) as usize,
            SeekFrom::Current(offset) => (self.position as i64 + offset) as usize,
        };

        if new_position > RAM_SIZE {
            return Err(FatError::InvalidInput);
        }

        self.position = new_position;
        Ok(self.position as u64)
    }
}

impl Read for &mut RamStorage {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let end = (self.position + buf.len()).min(RAM_SIZE);
        let len = end - self.position;
        buf[..len].copy_from_slice(&self.memory[self.position..self.position + len]);
        self.position = end;
        Ok(len)
    }
}

impl Write for &mut RamStorage {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let end = (self.position + buf.len()).min(RAM_SIZE);
        let len = end - self.position;
        self.memory[self.position..self.position + len].copy_from_slice(&buf[..len]);
        self.position = end;
        Ok(len)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Seek for RamStorage {
    fn seek(&mut self, pos: fatfs::SeekFrom) -> Result<u64, Self::Error> {
        let new_position = match pos {
            SeekFrom::Start(offset) => offset as usize,
            SeekFrom::End(offset) => (RAM_SIZE as i64 + offset) as usize,
            SeekFrom::Current(offset) => (self.position as i64 + offset) as usize,
        };

        if new_position > RAM_SIZE {
            return Err(FatError::InvalidInput);
        }

        self.position = new_position;
        Ok(self.position as u64)
    }
}


pub fn create_filesystem() -> Result<FileSystem<RamStorage>, fatfs::Error<()>> {

	serial_println!("Creating storage");
    let mut storage = RamStorage::new();

    serial_println!("Formatting storage");

    let _ = fatfs::format_volume(&mut storage, FormatVolumeOptions::new()).expect("Failed to format FS");

	serial_println!("Formatted storage");


    // Initialize the FAT file system
    let fs = FileSystem::new(storage, FsOptions::new()).expect("Failed to create FS");

	serial_println!("Returning fs");


    // You may need to create and format the file system if it's new
    // let _ = fs.format().map_err(FatError::from)?;

    Ok(fs)
}


lazy_static! {
	pub static ref FILE_SYSTEM: Mutex<FileSystem<RamStorage>> = Mutex::new(create_filesystem().unwrap());
}

