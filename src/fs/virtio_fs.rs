use fatfs::{IoBase, Read, Seek, SeekFrom, Write};
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::pci::PciTransport;
use crate::device::virtio_hal::OsHal;

pub struct VirtioBlockDevice {
    blk: VirtIOBlk<OsHal, PciTransport>,
    pos: u64,
    capacity_bytes: u64,
}

impl VirtioBlockDevice {
    pub fn new(blk: VirtIOBlk<OsHal, PciTransport>) -> Self {
        let capacity_bytes = blk.capacity() * 512;
        Self { blk, pos: 0, capacity_bytes }
    }
}

impl IoBase for VirtioBlockDevice {
    type Error = ();
}

impl Read for VirtioBlockDevice {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut total_read = 0;
        let mut remaining = buf;

        while !remaining.is_empty() {
            let sector = self.pos / 512;
            let offset = (self.pos % 512) as usize;
            let mut sector_buf = [0u8; 512];

            self.blk.read_blocks(sector as usize, &mut sector_buf).map_err(|_| ())?;

            let available = 512 - offset;
            let to_copy = available.min(remaining.len());
            remaining[..to_copy].copy_from_slice(&sector_buf[offset..offset + to_copy]);

            remaining = &mut remaining[to_copy..];
            self.pos += to_copy as u64;
            total_read += to_copy;
        }

        Ok(total_read)
    }
}

impl Write for VirtioBlockDevice {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let mut total_written = 0;
        let mut remaining = buf;

        while !remaining.is_empty() {
            let sector = self.pos / 512;
            let offset = (self.pos % 512) as usize;
            let mut sector_buf = [0u8; 512];

            // Read-modify-write for partial sectors
            self.blk.read_blocks(sector as usize, &mut sector_buf).map_err(|_| ())?;

            let available = 512 - offset;
            let to_copy = available.min(remaining.len());
            sector_buf[offset..offset + to_copy].copy_from_slice(&remaining[..to_copy]);

            self.blk.write_blocks(sector as usize, &sector_buf).map_err(|_| ())?;

            remaining = &remaining[to_copy..];
            self.pos += to_copy as u64;
            total_written += to_copy;
        }

        Ok(total_written)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Seek for VirtioBlockDevice {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        self.pos = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(offset) => (self.pos as i64 + offset) as u64,
            SeekFrom::End(offset) => (self.capacity_bytes as i64 + offset) as u64,
        };
        Ok(self.pos)
    }
}