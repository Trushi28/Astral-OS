//src/drivers/virtio/mod.rs
pub mod pci;
pub mod queue;
pub mod block;

use block::VirtioBlockDevice;
use pci::scan_for_virtio_block;
use spin::Mutex;
use alloc::boxed::Box;

static VIRTIO_BLK: Mutex<Option<Box<VirtioBlockDevice>>> = Mutex::new(None);

pub fn init() -> bool {
    match scan_for_virtio_block() {
        Some(pci_loc) => {
            crate::println!("      VirtIO-blk: Found at PCI {:02x}:{:02x}.{}", 
                pci_loc.bus, pci_loc.device, pci_loc.function);
            
            match VirtioBlockDevice::new(pci_loc) {
                Ok(device) => {
                    let capacity = device.capacity();
                    let capacity_mb = (capacity * 512) / (1024 * 1024);
                    
                    crate::println!("      VirtIO-blk: Initialized");
                    crate::println!("      Capacity: {} MB ({} sectors)", capacity_mb, capacity);
                    
                    *VIRTIO_BLK.lock() = Some(Box::new(device));
                    
                    // Test read
                    let mut test_buffer = [0u8; 512];
                    match disk_read_sector(0, &mut test_buffer) {
                        true => {
                            crate::println!("      Test read: OK");
                            true
                        }
                        false => {
                            crate::println!("      WARNING: Test read failed");
                            true // Still return true as device initialized
                        }
                    }
                }
                Err(e) => {
                    crate::println!("      VirtIO-blk: Initialization failed: {}", e);
                    false
                }
            }
        }
        None => {
            crate::println!("      VirtIO-blk: Not found");
            false
        }
    }
}

pub fn disk_read_sector(sector: u64, buffer: &mut [u8; 512]) -> bool {
    let mut dev = VIRTIO_BLK.lock();
    
    if let Some(ref mut device) = *dev {
        device.read_sector(sector, buffer).is_ok()
    } else {
        false
    }
}

pub fn disk_write_sector(sector: u64, buffer: &[u8; 512]) -> bool {
    let mut dev = VIRTIO_BLK.lock();
    
    if let Some(ref mut device) = *dev {
        device.write_sector(sector, buffer).is_ok()
    } else {
        false
    }
}

pub fn disk_is_present() -> bool {
    let dev = VIRTIO_BLK.lock();
    dev.as_ref().map(|d| d.is_initialized()).unwrap_or(false)
}

pub fn disk_get_capacity() -> u64 {
    let dev = VIRTIO_BLK.lock();
    dev.as_ref().map(|d| d.capacity()).unwrap_or(0)
}

pub fn disk_get_capacity_mb() -> u64 {
    (disk_get_capacity() * 512) / (1024 * 1024)
}