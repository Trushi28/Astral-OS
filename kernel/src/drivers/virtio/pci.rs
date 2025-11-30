//src/drivers/virtio/pci.rs
use crate::util::{inl, outl};

const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

const PCI_VENDOR_ID: u8 = 0x00;
const PCI_DEVICE_ID: u8 = 0x02;
const PCI_COMMAND: u8 = 0x04;
const PCI_BAR0: u8 = 0x10;
const PCI_CAPABILITY_LIST: u8 = 0x34;

const PCI_COMMAND_IO: u16 = 0x01;
const PCI_COMMAND_MEMORY: u16 = 0x02;
const PCI_COMMAND_MASTER: u16 = 0x04;

const VIRTIO_VENDOR_ID: u16 = 0x1AF4;
const VIRTIO_BLOCK_DEVICE_ID: u16 = 0x1042;

#[inline]
fn pci_config_address(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    (1u32 << 31)
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}

pub fn pci_config_read32(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let addr = pci_config_address(bus, device, func, offset);
    unsafe {
        outl(PCI_CONFIG_ADDR, addr);
        inl(PCI_CONFIG_DATA)
    }
}

pub fn pci_config_read16(bus: u8, device: u8, func: u8, offset: u8) -> u16 {
    let val = pci_config_read32(bus, device, func, offset & 0xFC);
    let shift = (offset & 2) * 8;
    ((val >> shift) & 0xFFFF) as u16
}

pub fn pci_config_read8(bus: u8, device: u8, func: u8, offset: u8) -> u8 {
    let val = pci_config_read32(bus, device, func, offset & 0xFC);
    let shift = (offset & 3) * 8;
    ((val >> shift) & 0xFF) as u8
}

pub fn pci_config_write32(bus: u8, device: u8, func: u8, offset: u8, value: u32) {
    let addr = pci_config_address(bus, device, func, offset);
    unsafe {
        outl(PCI_CONFIG_ADDR, addr);
        outl(PCI_CONFIG_DATA, value);
    }
}

pub fn pci_config_write16(bus: u8, device: u8, func: u8, offset: u8, value: u16) {
    let aligned_offset = offset & 0xFC;
    let mut val = pci_config_read32(bus, device, func, aligned_offset);
    let shift = (offset & 2) * 8;
    let mask = 0xFFFFu32 << shift;
    val = (val & !mask) | ((value as u32) << shift);
    pci_config_write32(bus, device, func, aligned_offset, val);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PciLocation {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PciLocation {
    pub const fn new(bus: u8, device: u8, function: u8) -> Self {
        Self { bus, device, function }
    }
    
    pub fn read32(&self, offset: u8) -> u32 {
        pci_config_read32(self.bus, self.device, self.function, offset)
    }
    
    pub fn read16(&self, offset: u8) -> u16 {
        pci_config_read16(self.bus, self.device, self.function, offset)
    }
    
    pub fn read8(&self, offset: u8) -> u8 {
        pci_config_read8(self.bus, self.device, self.function, offset)
    }
    
    pub fn write32(&self, offset: u8, value: u32) {
        pci_config_write32(self.bus, self.device, self.function, offset, value);
    }
    
    pub fn write16(&self, offset: u8, value: u16) {
        pci_config_write16(self.bus, self.device, self.function, offset, value);
    }
    
    pub fn enable_bus_mastering(&self) {
        let mut cmd = self.read16(PCI_COMMAND);
        cmd |= PCI_COMMAND_MEMORY | PCI_COMMAND_MASTER;
        self.write16(PCI_COMMAND, cmd);
    }
    
    pub fn read_bar(&self, bar_index: u8) -> u64 {
        assert!(bar_index < 6);
        let offset = PCI_BAR0 + (bar_index * 4);
        let bar_low = self.read32(offset);
        
        // Check if it's a 64-bit BAR
        if (bar_low & 0x04) != 0 {
            let bar_high = self.read32(offset + 4);
            ((bar_high as u64) << 32) | ((bar_low & !0xF) as u64)
        } else {
            (bar_low & !0xF) as u64
        }
    }
}

pub fn scan_for_virtio_block() -> Option<PciLocation> {
    for bus in 0..256u16 {
        for device in 0..32u8 {
            for function in 0..8u8 {
                let vendor = pci_config_read16(bus as u8, device, function, PCI_VENDOR_ID);
                
                if vendor == 0xFFFF {
                    if function == 0 {
                        break;
                    }
                    continue;
                }
                
                if vendor == VIRTIO_VENDOR_ID {
                    let device_id = pci_config_read16(bus as u8, device, function, PCI_DEVICE_ID);
                    
                    if device_id == VIRTIO_BLOCK_DEVICE_ID {
                        return Some(PciLocation::new(bus as u8, device, function));
                    }
                }
            }
        }
    }
    None
}