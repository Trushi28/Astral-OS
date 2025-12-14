//src/drivers/virtio/block.rs
use crate::sync::IrqSpinlock;
use super::pci::PciLocation;
use super::queue::Virtqueue;
use crate::memory::frame::{allocate_frame, deallocate_frame};
use crate::{PAGE_SIZE, get_hhdm_offset, get_timestamp};
use core::ptr::{write_volatile, read_volatile};
use core::sync::atomic::{fence, Ordering, AtomicBool};

const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;
const VIRTIO_BLK_S_OK: u8 = 0;
const VIRTIO_BLK_S_IOERR: u8 = 1;
const VIRTIO_BLK_S_UNSUPP: u8 = 2;

const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 1;
const VIRTIO_STATUS_DRIVER: u8 = 2;
const VIRTIO_STATUS_FEATURES_OK: u8 = 8;
const VIRTIO_STATUS_DRIVER_OK: u8 = 4;
const VIRTIO_STATUS_FAILED: u8 = 128;

const VIRTIO_F_VERSION_1: u64 = 1 << 32;

const VIRTIO_PCI_CAP_COMMON_CFG: u8 = 1;
const VIRTIO_PCI_CAP_NOTIFY_CFG: u8 = 2;
const VIRTIO_PCI_CAP_DEVICE_CFG: u8 = 4;
const VIRTIO_PCI_CAP_ISR_CFG: u8 = 3;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct VirtioBlkReq {
    req_type: u32,
    reserved: u32,
    sector: u64,
}

impl VirtioBlkReq {
    fn new_read(sector: u64) -> Self {
        Self { req_type: VIRTIO_BLK_T_IN, reserved: 0, sector }
    }
    
    fn new_write(sector: u64) -> Self {
        Self { req_type: VIRTIO_BLK_T_OUT, reserved: 0, sector }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtioBlkConfig {
    pub capacity: u64,
    pub size_max: u32,
    pub seg_max: u32,
    pub geometry_cylinders: u16,
    pub geometry_heads: u8,
    pub geometry_sectors: u8,
    pub blk_size: u32,
}

#[repr(u32)]
enum CommonCfgOffset {
    DeviceFeatureSelect = 0x00,
    DeviceFeature = 0x04,
    DriverFeatureSelect = 0x08,
    DriverFeature = 0x0C,
    ConfigMsixVector = 0x10,
    NumQueues = 0x12,
    DeviceStatus = 0x14,
    ConfigGeneration = 0x15,
    QueueSelect = 0x16,
    QueueSize = 0x18,
    QueueMsixVector = 0x1A,
    QueueEnable = 0x1C,
    QueueNotifyOff = 0x1E,
    QueueDesc = 0x20,
    QueueAvail = 0x28,
    QueueUsed = 0x30,
}

#[derive(Default)]
struct VirtioCapabilities {
    common_bar_addr: u64,
    notify_bar_addr: u64,
    device_bar_addr: u64,
    isr_bar_addr: u64,
    notify_off_multiplier: u32,
}

pub struct VirtioBlockDevice {
    pci_loc: PciLocation,
    common_cfg: usize,
    notify_base: usize,
    notify_off_multiplier: u32,
    device_cfg: usize,
    isr_cfg: usize,
    queue: Virtqueue,
    queue_notify_off: u16,
    capacity: u64,
    initialized: AtomicBool,
    device_lock: IrqSpinlock<()>,
}

unsafe impl Send for VirtioBlockDevice {}
unsafe impl Sync for VirtioBlockDevice {}

impl VirtioBlockDevice {
    pub fn new(pci_loc: PciLocation) -> Result<Self, &'static str> {
        pci_loc.enable_bus_mastering();
        
        let caps = Self::parse_capabilities(&pci_loc)?;
        
        if caps.common_bar_addr == 0 {
            return Err("Common config BAR not found");
        }
        
        let hhdm_offset = get_hhdm_offset();
        
        // Map MMIO regions BEFORE accessing them
        // This is required for UEFI where HHDM only covers RAM, not MMIO
        unsafe {
            let mut pt = crate::memory::PageTableManager::current();
            
            // Map each BAR region if present
            if caps.common_bar_addr != 0 {
                if let Err(e) = pt.map_mmio(caps.common_bar_addr, PAGE_SIZE) {
                    crate::serial_println!("[VIRTIO] Warning: Failed to map common BAR: {}", e);
                }
            }
            if caps.notify_bar_addr != 0 {
                if let Err(e) = pt.map_mmio(caps.notify_bar_addr, PAGE_SIZE) {
                    crate::serial_println!("[VIRTIO] Warning: Failed to map notify BAR: {}", e);
                }
            }
            if caps.device_bar_addr != 0 {
                if let Err(e) = pt.map_mmio(caps.device_bar_addr, PAGE_SIZE) {
                    crate::serial_println!("[VIRTIO] Warning: Failed to map device BAR: {}", e);
                }
            }
            if caps.isr_bar_addr != 0 {
                if let Err(e) = pt.map_mmio(caps.isr_bar_addr, PAGE_SIZE) {
                    crate::serial_println!("[VIRTIO] Warning: Failed to map ISR BAR: {}", e);
                }
            }
        }
        
        let common_cfg = (caps.common_bar_addr + hhdm_offset as u64) as usize;
        let notify_base = (caps.notify_bar_addr + hhdm_offset as u64) as usize;
        let device_cfg = (caps.device_bar_addr + hhdm_offset as u64) as usize;
        let isr_cfg = (caps.isr_bar_addr + hhdm_offset as u64) as usize;
        
        let mut device = Self {
            pci_loc,
            common_cfg,
            notify_base,
            notify_off_multiplier: caps.notify_off_multiplier,
            device_cfg,
            isr_cfg,
            queue: unsafe { core::mem::zeroed() },
            queue_notify_off: 0,
            capacity: 0,
            initialized: AtomicBool::new(false),
            device_lock: IrqSpinlock::new(()),
        };
        
        device.reset()?;
        device.negotiate_features()?;
        device.setup_queue()?;
        device.read_device_config()?;
        device.set_driver_ok()?;
        
        device.initialized.store(true, Ordering::SeqCst);
        Ok(device)
    }
    
    fn parse_capabilities(loc: &PciLocation) -> Result<VirtioCapabilities, &'static str> {
        let mut cap_ptr = loc.read8(0x34);
        
        if cap_ptr == 0 || cap_ptr == 0xFF {
            return Err("No capability list");
        }
        
        let mut caps = VirtioCapabilities::default();
        let mut iterations = 0;
        
        while cap_ptr != 0 && cap_ptr != 0xFF && iterations < 64 {
            iterations += 1;
            
            let cap_id = loc.read8(cap_ptr);
            
            if cap_id == 0x09 {
                let cfg_type = loc.read8(cap_ptr + 3);
                let bar = loc.read8(cap_ptr + 4);
                let offset = loc.read32(cap_ptr + 8);
                
                if bar >= 6 {
                    cap_ptr = loc.read8(cap_ptr + 1);
                    continue;
                }
                
                let bar_addr = loc.read_bar(bar);
                
                if bar_addr == 0 {
                    cap_ptr = loc.read8(cap_ptr + 1);
                    continue;
                }
                
                match cfg_type {
                    VIRTIO_PCI_CAP_COMMON_CFG => {
                        caps.common_bar_addr = bar_addr + offset as u64;
                    }
                    VIRTIO_PCI_CAP_NOTIFY_CFG => {
                        caps.notify_bar_addr = bar_addr + offset as u64;
                        caps.notify_off_multiplier = loc.read32(cap_ptr + 16);
                    }
                    VIRTIO_PCI_CAP_DEVICE_CFG => {
                        caps.device_bar_addr = bar_addr + offset as u64;
                    }
                    VIRTIO_PCI_CAP_ISR_CFG => {
                        caps.isr_bar_addr = bar_addr + offset as u64;
                    }
                    _ => {}
                }
            }
            
            cap_ptr = loc.read8(cap_ptr + 1);
        }
        
        if caps.common_bar_addr == 0 {
            return Err("Common config not found");
        }
        
        Ok(caps)
    }
    
    fn reset(&mut self) -> Result<(), &'static str> {
        self.write_device_status(0);
        
        for _ in 0..10000 {
            if self.read_device_status() == 0 {
                break;
            }
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
        
        self.write_device_status(VIRTIO_STATUS_ACKNOWLEDGE);
        
        let status = self.read_device_status();
        self.write_device_status(status | VIRTIO_STATUS_DRIVER);
        
        Ok(())
    }
    
    fn negotiate_features(&mut self) -> Result<(), &'static str> {
        self.write_common_u32(CommonCfgOffset::DeviceFeatureSelect, 0);
        let _features_low = self.read_common_u32(CommonCfgOffset::DeviceFeature);
        
        self.write_common_u32(CommonCfgOffset::DeviceFeatureSelect, 1);
        let _features_high = self.read_common_u32(CommonCfgOffset::DeviceFeature);
        
        let driver_features = VIRTIO_F_VERSION_1;
        
        self.write_common_u32(CommonCfgOffset::DriverFeatureSelect, 0);
        self.write_common_u32(CommonCfgOffset::DriverFeature, driver_features as u32);
        
        self.write_common_u32(CommonCfgOffset::DriverFeatureSelect, 1);
        self.write_common_u32(CommonCfgOffset::DriverFeature, (driver_features >> 32) as u32);
        
        let status = self.read_device_status();
        self.write_device_status(status | VIRTIO_STATUS_FEATURES_OK);
        
        for _ in 0..1000 {
            if self.read_device_status() & VIRTIO_STATUS_FEATURES_OK != 0 {
                return Ok(());
            }
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
        
        Err("Device rejected features")
    }
    
    fn setup_queue(&mut self) -> Result<(), &'static str> {
        self.write_common_u16(CommonCfgOffset::QueueSelect, 0);
        
        let max_queue_size = self.read_common_u16(CommonCfgOffset::QueueSize);
        
        if max_queue_size == 0 {
            return Err("Queue not available");
        }
        
        let mut queue_size = max_queue_size.min(128);
        
        loop {
            let (_, _, used_offset) = Virtqueue::size_requirements(queue_size);
            let used_size = 6 + (8 * queue_size as usize);
            let total_size = used_offset + used_size;
            
            if total_size <= PAGE_SIZE {
                break;
            }
            
            if queue_size <= 16 {
                return Err("Cannot fit queue in one page");
            }
            
            queue_size /= 2;
        }
        
        let frame = allocate_frame().ok_or("Out of memory")?;
        let queue_phys = frame.as_u64();
        let hhdm = get_hhdm_offset();
        let queue_virt = queue_phys as usize + hhdm;
        
        unsafe {
            core::ptr::write_bytes(queue_virt as *mut u8, 0, PAGE_SIZE);
        }
        
        self.queue = unsafe { Virtqueue::new(queue_virt, queue_phys, queue_size)? };
        
        let (desc_phys, avail_phys, used_phys) = self.queue.get_addresses();
        
        self.write_common_u16(CommonCfgOffset::QueueSize, queue_size);
        self.write_common_u64(CommonCfgOffset::QueueDesc, desc_phys);
        self.write_common_u64(CommonCfgOffset::QueueAvail, avail_phys);
        self.write_common_u64(CommonCfgOffset::QueueUsed, used_phys);
        
        self.queue_notify_off = self.read_common_u16(CommonCfgOffset::QueueNotifyOff);
        
        self.write_common_u16(CommonCfgOffset::QueueEnable, 1);
        
        let enabled = self.read_common_u16(CommonCfgOffset::QueueEnable);
        if enabled != 1 {
            return Err("Failed to enable queue");
        }
        
        Ok(())
    }
    
    fn read_device_config(&mut self) -> Result<(), &'static str> {
        unsafe {
            let config = self.device_cfg as *const VirtioBlkConfig;
            
            fence(Ordering::Acquire);
            self.capacity = read_volatile(&(*config).capacity);
            fence(Ordering::Release);
            
            if self.capacity == 0 {
                return Err("Device reported zero capacity");
            }
        }
        Ok(())
    }
    
    fn set_driver_ok(&mut self) -> Result<(), &'static str> {
        let status = self.read_device_status();
        self.write_device_status(status | VIRTIO_STATUS_DRIVER_OK);
        
        for _ in 0..1000 {
            let current_status = self.read_device_status();
            if current_status & VIRTIO_STATUS_FAILED != 0 {
                return Err("Device failed");
            }
            if current_status & VIRTIO_STATUS_DRIVER_OK != 0 {
                return Ok(());
            }
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
        
        Err("Device did not become ready")
    }
    
    #[inline]
    fn read_common_u8(&self, offset: CommonCfgOffset) -> u8 {
        unsafe {
            fence(Ordering::Acquire);
            let val = read_volatile((self.common_cfg + offset as usize) as *const u8);
            fence(Ordering::Release);
            val
        }
    }
    
    #[inline]
    fn write_common_u8(&self, offset: CommonCfgOffset, val: u8) {
        unsafe {
            fence(Ordering::Release);
            write_volatile((self.common_cfg + offset as usize) as *mut u8, val);
            fence(Ordering::Acquire);
        }
    }
    
    #[inline]
    fn read_common_u16(&self, offset: CommonCfgOffset) -> u16 {
        unsafe {
            fence(Ordering::Acquire);
            let val = read_volatile((self.common_cfg + offset as usize) as *const u16);
            fence(Ordering::Release);
            val
        }
    }
    
    #[inline]
    fn write_common_u16(&self, offset: CommonCfgOffset, val: u16) {
        unsafe {
            fence(Ordering::Release);
            write_volatile((self.common_cfg + offset as usize) as *mut u16, val);
            fence(Ordering::Acquire);
        }
    }
    
    #[inline]
    fn read_common_u32(&self, offset: CommonCfgOffset) -> u32 {
        unsafe {
            fence(Ordering::Acquire);
            let val = read_volatile((self.common_cfg + offset as usize) as *const u32);
            fence(Ordering::Release);
            val
        }
    }
    
    #[inline]
    fn write_common_u32(&self, offset: CommonCfgOffset, val: u32) {
        unsafe {
            fence(Ordering::Release);
            write_volatile((self.common_cfg + offset as usize) as *mut u32, val);
            fence(Ordering::Acquire);
        }
    }
    
    #[inline]
    fn write_common_u64(&self, offset: CommonCfgOffset, val: u64) {
        unsafe {
            fence(Ordering::Release);
            write_volatile((self.common_cfg + offset as usize) as *mut u64, val);
            fence(Ordering::Acquire);
        }
    }
    
    #[inline]
    fn read_device_status(&self) -> u8 {
        self.read_common_u8(CommonCfgOffset::DeviceStatus)
    }
    
    #[inline]
    fn write_device_status(&self, status: u8) {
        self.write_common_u8(CommonCfgOffset::DeviceStatus, status)
    }
    
    #[inline]
    fn notify_queue(&self) {
        let notify_addr = self.notify_base 
            + (self.queue_notify_off as usize * self.notify_off_multiplier as usize);
        
        unsafe {
            fence(Ordering::Release);
            write_volatile(notify_addr as *mut u16, 0);
            fence(Ordering::SeqCst);
        }
    }
    
    pub fn read_sector(&mut self, sector: u64, buffer: &mut [u8; 512]) -> Result<(), &'static str> {
        // Check initialization under lock, then drop lock before do_io
        {
            let _lock = self.device_lock.lock();
            if !self.initialized.load(Ordering::Acquire) {
                return Err("Device not initialized");
            }
        }
        // Lock is dropped here, now we can borrow self mutably
        self.do_io(sector, buffer, true)
    }

    pub fn write_sector(&mut self, sector: u64, buffer: &[u8; 512]) -> Result<(), &'static str> {
        // Check initialization under lock, then drop lock before do_io
        {
            let _lock = self.device_lock.lock();
            if !self.initialized.load(Ordering::Acquire) {
                return Err("Device not initialized");
            }
        }
        // Lock is dropped here, now we can borrow self mutably
        let mut temp = [0u8; 512];
        temp.copy_from_slice(buffer);
        self.do_io(sector, &mut temp, false)
    }
    
    fn do_io(&mut self, sector: u64, buffer: &mut [u8; 512], is_read: bool) -> Result<(), &'static str> {
        let head_desc = self.queue.alloc_desc().ok_or("No free descriptors")?;
        
        let header_frame = allocate_frame().ok_or("Out of memory")?;
        let data_frame = allocate_frame().ok_or("Out of memory")?;
        let status_frame = allocate_frame().ok_or("Out of memory")?;
        
        let hhdm = get_hhdm_offset();
        
        let header_phys = header_frame.as_u64();
        let header_virt = (header_phys as usize + hhdm) as *mut VirtioBlkReq;
        
        let data_phys = data_frame.as_u64();
        let data_virt = (data_phys as usize + hhdm) as *mut u8;
        
        let status_phys = status_frame.as_u64();
        let status_virt = (status_phys as usize + hhdm) as *mut u8;
        
        unsafe {
            let req = if is_read {
                VirtioBlkReq::new_read(sector)
            } else {
                VirtioBlkReq::new_write(sector)
            };
            write_volatile(header_virt, req);
            fence(Ordering::Release);
            
            if is_read {
                core::ptr::write_bytes(data_virt, 0, 512);
            } else {
                core::ptr::copy_nonoverlapping(buffer.as_ptr(), data_virt, 512);
            }
            fence(Ordering::Release);
            
            write_volatile(status_virt, 0xFF);
            fence(Ordering::Release);
        }
        
        let buffers = [
            (header_phys, 16u32, false),
            (data_phys, 512u32, is_read),
            (status_phys, 1u32, true),
        ];
        
        if let Err(e) = self.queue.add_buf(head_desc, &buffers) {
            self.queue.free_desc(head_desc);
            deallocate_frame(header_frame);
            deallocate_frame(data_frame);
            deallocate_frame(status_frame);
            return Err(e);
        }
        
        self.notify_queue();
        
        let start_time = get_timestamp();
        let timeout_ms = 5000;
        
        loop {
            fence(Ordering::Acquire);
            
            if let Some((completed_id, _len)) = self.queue.get_used() {
                if completed_id == head_desc {
                    let status = unsafe { read_volatile(status_virt) };
                    
                    if is_read && status == VIRTIO_BLK_S_OK {
                        unsafe {
                            core::ptr::copy_nonoverlapping(data_virt, buffer.as_mut_ptr(), 512);
                        }
                    }
                    
                    deallocate_frame(header_frame);
                    deallocate_frame(data_frame);
                    deallocate_frame(status_frame);
                    
                    match status {
                        VIRTIO_BLK_S_OK => return Ok(()),
                        VIRTIO_BLK_S_IOERR => return Err("I/O error"),
                        VIRTIO_BLK_S_UNSUPP => return Err("Unsupported operation"),
                        _ => return Err("Unknown error"),
                    }
                }
            }
            
            if get_timestamp() - start_time > timeout_ms {
                deallocate_frame(header_frame);
                deallocate_frame(data_frame);
                deallocate_frame(status_frame);
                return Err("Timeout");
            }
            
            unsafe { core::arch::asm!("pause", options(nomem, nostack)); }
        }
    }
    
    pub fn capacity(&self) -> u64 {
        self.capacity
    }
    
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }
}