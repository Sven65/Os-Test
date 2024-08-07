pub mod ahci;
pub mod scsi;
pub mod virtq;

use x86_64::instructions::port::Port;

use crate::serial_println;

const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

pub fn pci_config_read(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address = ((bus as u32) << 16)
                | ((device as u32) << 11)
                | ((function as u32) << 8)
                | ((offset as u32) & 0xfc)
                | 0x80000000;

    unsafe {
        let mut address_port = Port::new(PCI_CONFIG_ADDRESS);
        let mut data_port = Port::new(PCI_CONFIG_DATA);

        address_port.write(address);
        data_port.read()
    }
}

pub fn get_all_devices() {
    for bus in 0..=255 {
        for device in 0..32 {
            for function in 0..8 {
                let vendor_device_id = pci_config_read(bus, device, function, 0);
                let vendor_id = vendor_device_id & 0xFFFF;
                if vendor_id != 0xFFFF {
                    let device_id = (vendor_device_id >> 16) & 0xFFFF;
                    let class_code_reg = pci_config_read(bus, device, function, 8);
                    let class_code = (class_code_reg >> 24) & 0xFF;
                    let subclass_code = (class_code_reg >> 16) & 0xFF;
                    let prog_if = (class_code_reg >> 8) & 0xFF;

                    serial_println!(
                        "Found device: {:04x}:{:04x} (bus={}, device={}, function={}) - Class: {:02x}, Subclass: {:02x}, ProgIF: {:02x}",
                        vendor_id, device_id, bus, device, function, class_code, subclass_code, prog_if
                    );
                }
            }
        }
    }
}

#[cfg(test)]
#[test_case]
fn test() {
    assert!(1+1 == 2)
}
