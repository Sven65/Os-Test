use lazy_static::lazy_static;
use spin::Mutex;
use crate::serial_println;

const REG_CTRL:     u32 = 0x0000;
const REG_STATUS:   u32 = 0x0008;
const REG_EERD:     u32 = 0x0014;
const REG_IMC:      u32 = 0x00D8;
const REG_RCTL:     u32 = 0x0100;
const REG_TCTL:     u32 = 0x0400;
const REG_TIPG:     u32 = 0x0410;
const REG_RDBAL:    u32 = 0x2800;
const REG_RDBAH:    u32 = 0x2804;
const REG_RDLEN:    u32 = 0x2808;
const REG_RDH:      u32 = 0x2810;
const REG_RDT:      u32 = 0x2818;
const REG_TDBAL:    u32 = 0x3800;
const REG_TDBAH:    u32 = 0x3804;
const REG_TDLEN:    u32 = 0x3808;
const REG_TDH:      u32 = 0x3810;
const REG_TDT:      u32 = 0x3818;
const REG_RAL:      u32 = 0x5400;
const REG_RAH:      u32 = 0x5404;

const CTRL_RST:        u32 = 1 << 26;
const CTRL_SLU:        u32 = 1 << 6;
const CTRL_ASDE:       u32 = 1 << 5;
const RCTL_EN:         u32 = 1 << 1;
const RCTL_SBP:        u32 = 1 << 2;
const RCTL_UPE:        u32 = 1 << 3;
const RCTL_MPE:        u32 = 1 << 4;
const RCTL_BAM:        u32 = 1 << 15;
const RCTL_BSIZE_4096: u32 = (3 << 16) | (1 << 25);
const RCTL_SECRC:      u32 = 1 << 26;
const TCTL_EN:         u32 = 1 << 1;
const TCTL_PSP:        u32 = 1 << 3;
const TCTL_CT_SHIFT:   u32 = 4;
const TCTL_COLD_SHIFT: u32 = 12;
const RDESC_STATUS_DD: u8  = 1 << 0;
const TDESC_CMD_EOP:   u8  = 1 << 0;
const TDESC_CMD_IFCS:  u8  = 1 << 1;
const TDESC_CMD_RS:    u8  = 1 << 3;
const TDESC_STATUS_DD: u8  = 1 << 0;

const NUM_RX_DESC:    usize = 32;
const NUM_TX_DESC:    usize = 32;
const RX_BUFFER_SIZE: usize = 4096;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct RxDesc {
    addr:     u64,
    length:   u16,
    checksum: u16,
    status:   u8,
    errors:   u8,
    special:  u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct TxDesc {
    addr:    u64,
    length:  u16,
    cso:     u8,
    cmd:     u8,
    status:  u8,
    css:     u8,
    special: u16,
}

// Aligned statics for DMA — alignment is critical for e1000
#[repr(C, align(16))]
struct AlignedRxDescs([RxDesc; NUM_RX_DESC]);

#[repr(C, align(16))]
struct AlignedTxDescs([TxDesc; NUM_TX_DESC]);

#[repr(C, align(4096))]
struct AlignedRxBuffers([[u8; RX_BUFFER_SIZE]; NUM_RX_DESC]);

#[repr(C, align(4096))]
struct AlignedTxBuffers([[u8; 4096]; NUM_TX_DESC]);

static mut RX_DESCS: AlignedRxDescs = AlignedRxDescs([RxDesc {
    addr: 0, length: 0, checksum: 0, status: 0, errors: 0, special: 0,
}; NUM_RX_DESC]);

static mut TX_DESCS: AlignedTxDescs = AlignedTxDescs([TxDesc {
    addr: 0, length: 0, cso: 0, cmd: 0, status: 0, css: 0, special: 0,
}; NUM_TX_DESC]);

static mut RX_BUFFERS: AlignedRxBuffers = AlignedRxBuffers([[0u8; RX_BUFFER_SIZE]; NUM_RX_DESC]);
static mut TX_BUFFERS: AlignedTxBuffers = AlignedTxBuffers([[0u8; 4096]; NUM_TX_DESC]);

pub struct E1000 {
    base_virt: u64,
    rx_cur: usize,
    tx_cur: usize,
    pub mac: [u8; 6],
}

unsafe impl Send for E1000 {}
unsafe impl Sync for E1000 {}

lazy_static! {
    pub static ref E1000_DEV: Mutex<Option<E1000>> = Mutex::new(None);
}

pub fn init(phys_mem_offset: u64) {
    let e1000 = unsafe { E1000::new(0xfeb80000, phys_mem_offset) };
    *E1000_DEV.lock() = Some(e1000);
}

impl E1000 {
    pub unsafe fn new(base_phys: u64, phys_mem_offset: u64) -> Self {
        let base_virt = base_phys + phys_mem_offset;
        let mut e1000 = E1000 {
            base_virt,
            rx_cur: 0,
            tx_cur: 0,
            mac: [0u8; 6],
        };
        e1000.init();
        e1000
    }

    pub fn read_reg_pub(&self, offset: u32) -> u32 {
        self.read_reg(offset)
    }

    fn read_reg(&self, offset: u32) -> u32 {
        unsafe {
            core::ptr::read_volatile((self.base_virt + offset as u64) as *const u32)
        }
    }

    fn write_reg(&self, offset: u32, val: u32) {
        unsafe {
            core::ptr::write_volatile((self.base_virt + offset as u64) as *mut u32, val)
        }
    }

    fn read_eeprom(&self, addr: u8) -> u16 {
        self.write_reg(REG_EERD, 1 | ((addr as u32) << 2));
        loop {
            let val = self.read_reg(REG_EERD);
            if val & (1 << 1) != 0 {
                return (val >> 16) as u16;
            }
        }
    }

    fn read_mac(&mut self) {
        let ral = self.read_reg(REG_RAL);
        let rah = self.read_reg(REG_RAH);
        if rah & (1 << 31) != 0 {
            self.mac[0] = (ral & 0xFF) as u8;
            self.mac[1] = ((ral >> 8) & 0xFF) as u8;
            self.mac[2] = ((ral >> 16) & 0xFF) as u8;
            self.mac[3] = ((ral >> 24) & 0xFF) as u8;
            self.mac[4] = (rah & 0xFF) as u8;
            self.mac[5] = ((rah >> 8) & 0xFF) as u8;
            return;
        }
        let low  = self.read_eeprom(0);
        let mid  = self.read_eeprom(1);
        let high = self.read_eeprom(2);
        self.mac[0] = (low & 0xFF) as u8;
        self.mac[1] = (low >> 8) as u8;
        self.mac[2] = (mid & 0xFF) as u8;
        self.mac[3] = (mid >> 8) as u8;
        self.mac[4] = (high & 0xFF) as u8;
        self.mac[5] = (high >> 8) as u8;
    }

    unsafe fn init(&mut self) {
        self.write_reg(REG_CTRL, self.read_reg(REG_CTRL) | CTRL_RST);
        for _ in 0..100_000 { core::hint::spin_loop(); }
        self.write_reg(REG_CTRL, CTRL_ASDE | CTRL_SLU);
        self.write_reg(REG_IMC, 0xFFFFFFFF);
        self.read_mac();
        serial_println!("[e1000] MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.mac[0], self.mac[1], self.mac[2],
            self.mac[3], self.mac[4], self.mac[5]);

        let ral = (self.mac[0] as u32) | ((self.mac[1] as u32) << 8)
            | ((self.mac[2] as u32) << 16) | ((self.mac[3] as u32) << 24);
        let rah = (self.mac[4] as u32) | ((self.mac[5] as u32) << 8) | (1 << 31);
        self.write_reg(REG_RAL, ral);
        self.write_reg(REG_RAH, rah);

        self.init_rx();
        self.init_tx();

        serial_println!("[e1000] Initialized, status={:#x}", self.read_reg(REG_STATUS));
    }

    unsafe fn init_rx(&mut self) {
        let descs = &mut RX_DESCS.0;
        let buffers = &RX_BUFFERS.0;

        for i in 0..NUM_RX_DESC {
            let buf_virt = buffers[i].as_ptr() as u64;
            let buf_phys = crate::device::virtio_hal::virt_to_phys_pub(buf_virt);
            descs[i].addr = buf_phys;
            descs[i].status = 0;
        }

        let ring_virt = descs.as_ptr() as u64;
        let ring_phys = crate::device::virtio_hal::virt_to_phys_pub(ring_virt);
        serial_println!("[e1000] RX ring phys={:#x} (aligned={})",
            ring_phys, ring_phys % 16 == 0);

        self.write_reg(REG_RDBAH, (ring_phys >> 32) as u32);
        self.write_reg(REG_RDBAL, ring_phys as u32);
        self.write_reg(REG_RDLEN, (NUM_RX_DESC * core::mem::size_of::<RxDesc>()) as u32);
        self.write_reg(REG_RDH, 0);
        self.write_reg(REG_RDT, (NUM_RX_DESC - 1) as u32);
        self.write_reg(REG_RCTL,
                       RCTL_EN | RCTL_SBP | RCTL_UPE | RCTL_MPE | RCTL_BAM | RCTL_BSIZE_4096 | RCTL_SECRC);
    }

    unsafe fn init_tx(&mut self) {
        let descs = &mut TX_DESCS.0;
        for i in 0..NUM_TX_DESC {
            descs[i].status = TDESC_STATUS_DD;
        }

        let ring_virt = descs.as_ptr() as u64;
        let ring_phys = crate::device::virtio_hal::virt_to_phys_pub(ring_virt);
        serial_println!("[e1000] TX ring phys={:#x} (aligned={})",
            ring_phys, ring_phys % 16 == 0);

        self.write_reg(REG_TDBAH, (ring_phys >> 32) as u32);
        self.write_reg(REG_TDBAL, ring_phys as u32);
        self.write_reg(REG_TDLEN, (NUM_TX_DESC * core::mem::size_of::<TxDesc>()) as u32);
        self.write_reg(REG_TDH, 0);
        self.write_reg(REG_TDT, 0);
        self.write_reg(REG_TCTL,
                       TCTL_EN | TCTL_PSP | (15 << TCTL_CT_SHIFT) | (63 << TCTL_COLD_SHIFT));
        self.write_reg(REG_TIPG, 0x0060200A);
    }

    pub fn send(&mut self, data: &[u8]) -> bool {
        let descs = unsafe { &mut TX_DESCS.0 };
        let buffers = unsafe { &mut TX_BUFFERS.0 };
        let desc = &mut descs[self.tx_cur];

        let mut timeout = 100000;
        while desc.status & TDESC_STATUS_DD == 0 {
            core::hint::spin_loop();
            timeout -= 1;
            if timeout == 0 {
                serial_println!("[e1000] TX timeout");
                return false;
            }
        }

        let buf = &mut buffers[self.tx_cur];
        let len = data.len().min(4096);
        buf[..len].copy_from_slice(&data[..len]);

        let buf_phys = crate::device::virtio_hal::virt_to_phys_pub(buf.as_ptr() as u64);
        desc.addr = buf_phys;
        desc.length = len as u16;
        desc.cmd = TDESC_CMD_EOP | TDESC_CMD_IFCS | TDESC_CMD_RS;
        desc.status = 0;

        self.tx_cur = (self.tx_cur + 1) % NUM_TX_DESC;
        self.write_reg(REG_TDT, self.tx_cur as u32);
        true
    }

    pub fn recv(&mut self, buf: &mut [u8]) -> Option<usize> {
        let descs = unsafe { &mut RX_DESCS.0 };
        let buffers = unsafe { &RX_BUFFERS.0 };
        let desc = &mut descs[self.rx_cur];

        if desc.status & RDESC_STATUS_DD == 0 {
            return None;
        }

        let len = desc.length as usize;
        let src = &buffers[self.rx_cur][..len];
        let copy_len = len.min(buf.len());
        buf[..copy_len].copy_from_slice(&src[..copy_len]);

        desc.status = 0;
        let buf_virt = buffers[self.rx_cur].as_ptr() as u64;
        let buf_phys = crate::device::virtio_hal::virt_to_phys_pub(buf_virt);
        desc.addr = buf_phys;
        self.write_reg(REG_RDT, self.rx_cur as u32);
        self.rx_cur = (self.rx_cur + 1) % NUM_RX_DESC;

        Some(len)
    }
}