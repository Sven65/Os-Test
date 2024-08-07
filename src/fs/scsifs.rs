use alloc::vec;
use fatfs::{FileSystem, FormatVolumeOptions, FsOptions, IoBase, Read, Seek, Write};
use core::ptr;

use crate::{println, serial_println};

pub struct SCSIBlockDevice {
    base_addr: u64,
    block_size: u64, // Define block size if necessary
    current_pos: u64, // Keep track of the current position for Seek
}

impl SCSIBlockDevice {
    pub fn new(base_addr: u64, block_size: u64) -> Self {
        SCSIBlockDevice {
            base_addr,
            block_size,
            current_pos: 0,
        }
    }

    fn issue_scsi_command(&self, command: &[u8], buffer: &mut [u8]) -> Result<(), ()> {
        // Step 1: Write Command
        // Example assumes base_addr points to the SCSI command register
        let command_register = self.base_addr; // Adjust based on actual hardware interface
        unsafe {
            ptr::copy_nonoverlapping(command.as_ptr(), command_register as *mut u8, command.len());
        }
    
        // Step 2: Issue the Command
        // Example assumes there is a command trigger register
        // let command_trigger_register = ...;
        // unsafe { ptr::write_volatile(command_trigger_register as *mut u8, 1); } // Trigger command
    
        // Step 3: Wait for Completion
        // Polling a status register or checking an interrupt
        // let status_register = ...;
        // while unsafe { ptr::read_volatile(status_register as *const u8) } != 0 {
        //     // Wait until the command completes
        // }
    
        // Step 4: Transfer Data
        // For a read command, copy data from the device to the buffer
        if buffer.len() > 0 {
            unsafe {
                ptr::copy_nonoverlapping(command_register as *const u8, buffer.as_mut_ptr(), buffer.len());
            }
        }
    
        // For a write command, copy data from the buffer to the device
        // Example assumes base_addr points to a data register
        if buffer.len() > 0 {
            unsafe {
                ptr::copy_nonoverlapping(buffer.as_ptr(), command_register as *mut u8, buffer.len());
            }
        }
    
        Ok(())
    }

    fn read_block(&self, block: u64, buffer: &mut [u8]) -> Result<(), ()> {
        // Define the SCSI READ (10) command
        let command = [
            0x28, // READ (10) opcode
            0x00, // Reserved
            0x00, // Reserved
            0x00, // Reserved
            (block >> 24) as u8, // MSB of LBA
            (block >> 16) as u8,
            (block >> 8) as u8,
            (block & 0xFF) as u8, // LSB of LBA
            0x00, // Reserved
            0x00, // Transfer length MSB
            (buffer.len() >> 9) as u8, // Transfer length LSB
            0x00  // Control
        ];
    
        // Send the SCSI command to the device
        // Replace this with your actual SCSI I/O operation
        let result = self.issue_scsi_command(&command, buffer);
        result.map_err(|e| {
            // Log or handle the error
            println!("SCSI read error: {:?}", e);
            ()
        })
    }
    

    fn write_block(&self, block: u64, data: &mut [u8]) -> Result<(), ()> {
        // Define the SCSI WRITE (10) command
        let command = [
            0x2A, // WRITE (10) opcode
            0x00, // Reserved
            0x00, // Reserved
            0x00, // Reserved
            (block >> 24) as u8, // MSB of LBA
            (block >> 16) as u8,
            (block >> 8) as u8,
            (block & 0xFF) as u8, // LSB of LBA
            0x00, // Reserved
            0x00, // Transfer length MSB
            (data.len() >> 9) as u8, // Transfer length LSB
            0x00  // Control
        ];
    
        // Send the SCSI command to the device with data
        let result = self.issue_scsi_command(&command, data);
        result.map_err(|e| {
            // Log or handle the error
            println!("SCSI write error: {:?}", e);
            ()
        })
    }
}


impl IoBase for SCSIBlockDevice {
    type Error = fatfs::Error<()>;
}

impl Read for SCSIBlockDevice {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let block = self.current_pos / self.block_size;
        let offset = (self.current_pos % self.block_size) as usize;
        let read_len = buf.len().min(self.block_size as usize - offset);
        let mut block_buffer = vec![0; self.block_size as usize];
        
        self.read_block(block, &mut block_buffer).map_err(|_| fatfs::Error::Io(()))?;
        
        buf[..read_len].copy_from_slice(&block_buffer[offset..offset + read_len]);
        self.current_pos += read_len as u64;
        Ok(read_len)
    }
}

impl Write for SCSIBlockDevice {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let block = self.current_pos / self.block_size;
        let offset = (self.current_pos % self.block_size) as usize;
        let write_len = buf.len().min(self.block_size as usize - offset);
        let mut block_buffer = vec![0; self.block_size as usize];
        
        self.read_block(block, &mut block_buffer).map_err(|_| fatfs::Error::Io(()))?;
        
        block_buffer[offset..offset + write_len].copy_from_slice(&buf[..write_len]);
        self.write_block(block, &mut block_buffer).map_err(|_| fatfs::Error::Io(()))?;
        self.current_pos += write_len as u64;
        Ok(write_len)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        // Typically flush is a no-op for block devices
        Ok(())
    }
}

impl Seek for SCSIBlockDevice {
    fn seek(&mut self, pos: fatfs::SeekFrom) -> Result<u64, Self::Error> {
        // match pos {
        //     fatfs::SeekFrom::Start(offset) => {
        //         self.current_pos = offset;
        //     }
        //     fatfs::SeekFrom::Current(offset) => {
        //         self.current_pos = self.current_pos.checked_add(offset).ok_or(fatfs::Error::Io(()))?;
        //     }
        //     fatfs::SeekFrom::End(offset) => {
        //         // For end-relative seeks, you might need to know the size of the device
        //         // Assuming a hypothetical `get_device_size` method
        //         let device_size = self.get_device_size(); // Implement this method
        //         self.current_pos = device_size.checked_add(offset).ok_or(fatfs::Error::Io(()))?;
        //     }
        // }
        // Ok(self.current_pos)
        Ok(0)
    }
}

// pub fn create_fs(
//     base_addr: u64,
//     block_size: u64,
// ) -> Result<FileSystem<SCSIBlockDevice>, fatfs::Error<()>> {
//     serial_println!("Creating storage");
//     let mut storage: SCSIBlockDevice = SCSIBlockDevice::new(
//         base_addr,
//         block_size,
//     );

//     serial_println!("Formatting storage");

//     let _ = fatfs::format_volume(&mut storage, FormatVolumeOptions::new()).expect("Failed to format FS");

// 	serial_println!("Formatted storage");


//     // Initialize the FAT file system
//     let fs = FileSystem::new(storage, FsOptions::new()).expect("Failed to create FS");

// 	serial_println!("Returning fs");


//     // You may need to create and format the file system if it's new
//     // let _ = fs.format().map_err(FatError::from)?;

//     Ok(fs)
// }

pub fn create_fs(
    base_addr: u64,
    block_size: u64,
) -> Result<FileSystem<SCSIBlockDevice>, fatfs::Error<()>> {
    serial_println!("Creating storage");
    let mut storage: SCSIBlockDevice = SCSIBlockDevice::new(
        base_addr,
        block_size,
    );

    serial_println!("Block size: {}", block_size);
    serial_println!("Formatting storage");

    // Ensure block size is within acceptable range
    if block_size < 512 || block_size > 4096 {
        return Err(fatfs::Error::InvalidInput); // Adjust based on acceptable ranges
    }

    match fatfs::format_volume(&mut storage, FormatVolumeOptions::new()) {
        Err(e) => { serial_println!("Formatting error: {:#?}", e); }
        Ok(_) => { serial_println!("Formatted storage."); }
    }

    serial_println!("Formatted storage");

    // Initialize the FAT file system
    let fs = FileSystem::new(storage, FsOptions::new());

    if fs.is_err() {
        serial_println!("Failed to init fs: {:#?}", fs.err());
        Err(fatfs::Error::Io(()))
    } else {
        serial_println!("Returning fs");

        Ok(fs.ok().unwrap())
    }
}