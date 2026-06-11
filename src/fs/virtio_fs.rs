use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use fatfs::{IoBase, Read, Seek, SeekFrom, Write};
use virtio_drivers::device::blk::VirtIOBlk;
use virtio_drivers::transport::pci::PciTransport;
use crate::device::virtio_hal::OsHal;

const SECTOR_SIZE: usize = 512;
const CACHE_MAX_SECTORS: usize = 64; // 32KB cache

struct CachedSector {
    data: [u8; SECTOR_SIZE],
    dirty: bool,
    last_used: u64,
}

pub struct VirtioBlockDevice {
    blk: VirtIOBlk<OsHal, PciTransport>,
    pos: u64,
    capacity_bytes: u64,
    cache: BTreeMap<u64, CachedSector>,
    clock: u64,
}

impl VirtioBlockDevice {
    pub fn new(blk: VirtIOBlk<OsHal, PciTransport>) -> Self {
        let capacity_bytes = blk.capacity() * 512;
        Self {
            blk,
            pos: 0,
            capacity_bytes,
            cache: BTreeMap::new(),
            clock: 0,
        }
    }

    /// Get a sector into cache (reading from disk on miss), return mutable ref.
    fn sector(&mut self, sector: u64) -> Result<&mut CachedSector, ()> {
        self.clock += 1;
        let clock = self.clock;

        if !self.cache.contains_key(&sector) {
            self.evict_if_full()?;
            let mut data = [0u8; SECTOR_SIZE];
            self.blk.read_blocks(sector as usize, &mut data).map_err(|_| ())?;
            self.cache.insert(sector, CachedSector { data, dirty: false, last_used: clock });
        }

        let entry = self.cache.get_mut(&sector).unwrap();
        entry.last_used = clock;
        Ok(entry)
    }

    fn evict_if_full(&mut self) -> Result<(), ()> {
        while self.cache.len() >= CACHE_MAX_SECTORS {
            // find least-recently-used
            let lru = self.cache.iter()
                .min_by_key(|(_, e)| e.last_used)
                .map(|(&s, _)| s)
                .ok_or(())?;
            if let Some(entry) = self.cache.remove(&lru) {
                if entry.dirty {
                    self.blk.write_blocks(lru as usize, &entry.data).map_err(|_| ())?;
                }
            }
        }
        Ok(())
    }

    /// Write all dirty sectors to disk. Call after logical operations complete.
    pub fn flush_cache(&mut self) -> Result<(), ()> {
        let dirty: Vec<u64> = self.cache.iter()
            .filter(|(_, e)| e.dirty)
            .map(|(&s, _)| s)
            .collect();
        for sector in dirty {
            let entry = self.cache.get_mut(&sector).unwrap();
            self.blk.write_blocks(sector as usize, &entry.data).map_err(|_| ())?;
            entry.dirty = false;
        }
        Ok(())
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

            let cached = self.sector(sector)?;
            let available = SECTOR_SIZE - offset;
            let to_copy = available.min(remaining.len());
            remaining[..to_copy].copy_from_slice(&cached.data[offset..offset + to_copy]);

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

            let cached = self.sector(sector)?;
            let available = SECTOR_SIZE - offset;
            let to_copy = available.min(remaining.len());
            cached.data[offset..offset + to_copy].copy_from_slice(&remaining[..to_copy]);
            cached.dirty = true;

            remaining = &remaining[to_copy..];
            self.pos += to_copy as u64;
            total_written += to_copy;
        }

        Ok(total_written)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.flush_cache()
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