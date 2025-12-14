// src/memory/paging.rs
use super::frame::{PhysAddr, allocate_frame};
use crate::PAGE_SIZE;
use core::arch::asm;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const PRESENT: u64 = 1 << 0;
    pub const WRITABLE: u64 = 1 << 1;
    pub const USER: u64 = 1 << 2;
    pub const WRITE_THROUGH: u64 = 1 << 3;
    pub const NO_CACHE: u64 = 1 << 4;
    pub const ACCESSED: u64 = 1 << 5;
    pub const DIRTY: u64 = 1 << 6;
    pub const HUGE: u64 = 1 << 7;
    pub const GLOBAL: u64 = 1 << 8;
    pub const NO_EXECUTE: u64 = 1 << 63;
    
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }
    
    #[inline]
    pub fn is_present(self) -> bool {
        self.0 & Self::PRESENT != 0
    }
    
    #[inline]
    pub fn set_present(&mut self, present: bool) {
        if present {
            self.0 |= Self::PRESENT;
        } else {
            self.0 &= !Self::PRESENT;
        }
    }
    
    #[inline]
    pub fn physical_address(self) -> u64 {
        self.0 & 0x000F_FFFF_FFFF_F000
    }
    
    #[inline]
    pub fn set_address(&mut self, addr: u64, flags: u64) {
        // Combine physical address with flags, including high bits (NX at bit 63)
        self.0 = (addr & 0x000F_FFFF_FFFF_F000) | (flags & 0xFFF) | (flags & Self::NO_EXECUTE);
    }
    
    #[inline]
    pub fn flags(self) -> u64 {
        // Return both low flags and NX bit
        (self.0 & 0xFFF) | (self.0 & Self::NO_EXECUTE)
    }
}

#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; 512],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::new(); 512],
        }
    }
    
    #[inline]
    pub fn zero(&mut self) {
        unsafe {
            core::ptr::write_bytes(self as *mut _, 0, 1);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VirtAddr(u64);

impl VirtAddr {
    #[inline]
    pub const fn new(addr: u64) -> Self {
        Self(addr)
    }
    
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
    
    #[inline]
    pub const fn p4_index(self) -> usize {
        ((self.0 >> 39) & 0x1FF) as usize
    }
    
    #[inline]
    pub const fn p3_index(self) -> usize {
        ((self.0 >> 30) & 0x1FF) as usize
    }
    
    #[inline]
    pub const fn p2_index(self) -> usize {
        ((self.0 >> 21) & 0x1FF) as usize
    }
    
    #[inline]
    pub const fn p1_index(self) -> usize {
        ((self.0 >> 12) & 0x1FF) as usize
    }
    
    #[inline]
    pub const fn page_offset(self) -> usize {
        (self.0 & 0xFFF) as usize
    }
}

pub struct PageTableManager {
    p4_table: &'static mut PageTable,
}

impl PageTableManager {
    /// Get current page table
    pub unsafe fn current() -> Self {
        let p4_addr = Self::read_cr3();
        let hhdm = crate::get_hhdm_offset();
        let p4_ptr = ((p4_addr as usize) + hhdm) as *mut PageTable;
        
        Self {
            p4_table: &mut *p4_ptr,
        }
    }
    
    /// Create new page table
    pub fn new() -> Option<Self> {
        let frame = allocate_frame()?;
        let p4_ptr = frame.to_virt() as *mut PageTable;
        
        unsafe {
            (*p4_ptr).zero();
            
            Some(Self {
                p4_table: &mut *p4_ptr,
            })
        }
    }
    
    /// Get physical address of P4 table
    pub fn p4_physical(&self) -> PhysAddr {
        let hhdm = crate::get_hhdm_offset();
        let virt = self.p4_table as *const _ as usize;
        PhysAddr::new((virt - hhdm) as u64)
    }
    
    /// Map virtual page to physical frame
    pub fn map(&mut self, virt: VirtAddr, phys: PhysAddr, flags: u64) -> Result<(), &'static str> {
        let p4_index = virt.p4_index();
        let p3_index = virt.p3_index();
        let p2_index = virt.p2_index();
        let p1_index = virt.p1_index();
        
        let hhdm = crate::get_hhdm_offset();
        
        // Get or create P3
        let p3_entry = &mut self.p4_table.entries[p4_index];
        let p3_phys = if p3_entry.is_present() {
            p3_entry.physical_address()
        } else {
            let frame = allocate_frame().ok_or("Out of memory")?;
            let ptr = frame.to_virt() as *mut PageTable;
            unsafe { (*ptr).zero(); }
            let phys = frame.as_u64();
            p3_entry.set_address(
                phys,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER
            );
            phys
        };
        
        let p3 = unsafe { &mut *((p3_phys as usize + hhdm) as *mut PageTable) };
        
        // Get or create P2
        let p2_entry = &mut p3.entries[p3_index];
        let p2_phys = if p2_entry.is_present() {
            p2_entry.physical_address()
        } else {
            let frame = allocate_frame().ok_or("Out of memory")?;
            let ptr = frame.to_virt() as *mut PageTable;
            unsafe { (*ptr).zero(); }
            let phys = frame.as_u64();
            p2_entry.set_address(
                phys,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER
            );
            phys
        };
        
        let p2 = unsafe { &mut *((p2_phys as usize + hhdm) as *mut PageTable) };
        
        // Get or create P1
        let p1_entry = &mut p2.entries[p2_index];
        let p1_phys = if p1_entry.is_present() {
            p1_entry.physical_address()
        } else {
            let frame = allocate_frame().ok_or("Out of memory")?;
            let ptr = frame.to_virt() as *mut PageTable;
            unsafe { (*ptr).zero(); }
            let phys = frame.as_u64();
            p1_entry.set_address(
                phys,
                PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER
            );
            phys
        };
        
        let p1 = unsafe { &mut *((p1_phys as usize + hhdm) as *mut PageTable) };
        
        // Map the page
        let entry = &mut p1.entries[p1_index];
        if entry.is_present() {
            return Err("Page already mapped");
        }
        
        entry.set_address(phys.as_u64(), flags);
        
        // Flush TLB
        unsafe {
            asm!("invlpg [{}]", in(reg) virt.as_u64(), options(nostack, preserves_flags));
        }
        
        Ok(())
    }
    
    /// Map an executable page - clears NX at all levels
    pub fn map_executable(&mut self, virt: VirtAddr, phys: PhysAddr, flags: u64) -> Result<(), &'static str> {
        let p4_index = virt.p4_index();
        let p3_index = virt.p3_index();
        let p2_index = virt.p2_index();
        let p1_index = virt.p1_index();
        
        let hhdm = crate::get_hhdm_offset();
        
        // Flags for intermediate entries: Present + Writable + User, NO NX
        let inter_flags = PageTableEntry::PRESENT | PageTableEntry::WRITABLE | PageTableEntry::USER;
        
        // Get or create P3 - force clear NX
        let p3_entry = &mut self.p4_table.entries[p4_index];
        let p3_phys = if p3_entry.is_present() {
            let addr = p3_entry.physical_address();
            // Force clear NX bit on existing entry
            p3_entry.set_address(addr, inter_flags);
            addr
        } else {
            let frame = allocate_frame().ok_or("Out of memory")?;
            let ptr = frame.to_virt() as *mut PageTable;
            unsafe { (*ptr).zero(); }
            let phys = frame.as_u64();
            p3_entry.set_address(phys, inter_flags);
            phys
        };
        
        let p3 = unsafe { &mut *((p3_phys as usize + hhdm) as *mut PageTable) };
        
        // Get or create P2 - force clear NX
        let p2_entry = &mut p3.entries[p3_index];
        let p2_phys = if p2_entry.is_present() {
            let addr = p2_entry.physical_address();
            p2_entry.set_address(addr, inter_flags);
            addr
        } else {
            let frame = allocate_frame().ok_or("Out of memory")?;
            let ptr = frame.to_virt() as *mut PageTable;
            unsafe { (*ptr).zero(); }
            let phys = frame.as_u64();
            p2_entry.set_address(phys, inter_flags);
            phys
        };
        
        let p2 = unsafe { &mut *((p2_phys as usize + hhdm) as *mut PageTable) };
        
        // Get or create P1 - force clear NX
        let p1_entry = &mut p2.entries[p2_index];
        let p1_phys = if p1_entry.is_present() {
            let addr = p1_entry.physical_address();
            p1_entry.set_address(addr, inter_flags);
            addr
        } else {
            let frame = allocate_frame().ok_or("Out of memory")?;
            let ptr = frame.to_virt() as *mut PageTable;
            unsafe { (*ptr).zero(); }
            let phys = frame.as_u64();
            p1_entry.set_address(phys, inter_flags);
            phys
        };
        
        let p1 = unsafe { &mut *((p1_phys as usize + hhdm) as *mut PageTable) };
        
        // Map the page - explicitly no NX
        let entry = &mut p1.entries[p1_index];
        // Force overwrite even if present
        entry.set_address(phys.as_u64(), flags & !PageTableEntry::NO_EXECUTE);
        
        // Flush TLB
        unsafe {
            asm!("invlpg [{}]", in(reg) virt.as_u64(), options(nostack, preserves_flags));
        }
        
        Ok(())
    }
    
    /// Map an MMIO physical address range into the current page table
    /// This maps physical address to virtual address using HHDM convention
    /// MMIO pages are marked as uncached and no-execute
    pub fn map_mmio(&mut self, phys_addr: u64, size: usize) -> Result<(), &'static str> {
        let hhdm = crate::get_hhdm_offset() as u64;
        let num_pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        
        // MMIO mapping: phys -> hhdm + phys (so HHDM access works)
        for i in 0..num_pages {
            let page_phys = (phys_addr & !0xFFF) + (i * PAGE_SIZE) as u64;
            let page_virt = VirtAddr::new(page_phys + hhdm);
            let phys = PhysAddr::new(page_phys);
            
            // Check if already mapped
            if self.translate(page_virt).is_some() {
                continue; // Already mapped, skip
            }
            
            // Map with MMIO-appropriate flags: present, writable, no cache, no execute
            let flags = PageTableEntry::PRESENT 
                | PageTableEntry::WRITABLE 
                | PageTableEntry::NO_CACHE 
                | PageTableEntry::WRITE_THROUGH
                | PageTableEntry::NO_EXECUTE;
            
            self.map(page_virt, phys, flags)?;
        }
        
        Ok(())
    }
    
    /// Unmap virtual page
    pub fn unmap(&mut self, virt: VirtAddr) -> Result<PhysAddr, &'static str> {
        let p4_index = virt.p4_index();
        let p3_index = virt.p3_index();
        let p2_index = virt.p2_index();
        let p1_index = virt.p1_index();
        
        let hhdm = crate::get_hhdm_offset();
        
        let p3_entry = &self.p4_table.entries[p4_index];
        if !p3_entry.is_present() {
            return Err("P3 not present");
        }
        let p3 = unsafe { &*((p3_entry.physical_address() as usize + hhdm) as *const PageTable) };
        
        let p2_entry = &p3.entries[p3_index];
        if !p2_entry.is_present() {
            return Err("P2 not present");
        }
        let p2 = unsafe { &*((p2_entry.physical_address() as usize + hhdm) as *const PageTable) };
        
        let p1_entry = &p2.entries[p2_index];
        if !p1_entry.is_present() {
            return Err("P1 not present");
        }
        let p1 = unsafe { &mut *((p1_entry.physical_address() as usize + hhdm) as *mut PageTable) };
        
        let entry = &mut p1.entries[p1_index];
        if !entry.is_present() {
            return Err("Page not mapped");
        }
        
        let phys = PhysAddr::new(entry.physical_address());
        entry.set_present(false);
        
        unsafe {
            asm!("invlpg [{}]", in(reg) virt.as_u64(), options(nostack, preserves_flags));
        }
        
        Ok(phys)
    }
    
    /// Translate virtual to physical
    pub fn translate(&self, virt: VirtAddr) -> Option<PhysAddr> {
        let hhdm = crate::get_hhdm_offset();
        
        let p3_entry = &self.p4_table.entries[virt.p4_index()];
        if !p3_entry.is_present() {
            return None;
        }
        let p3 = unsafe { &*((p3_entry.physical_address() as usize + hhdm) as *const PageTable) };
        
        let p2_entry = &p3.entries[virt.p3_index()];
        if !p2_entry.is_present() {
            return None;
        }
        let p2 = unsafe { &*((p2_entry.physical_address() as usize + hhdm) as *const PageTable) };
        
        let p1_entry = &p2.entries[virt.p2_index()];
        if !p1_entry.is_present() {
            return None;
        }
        let p1 = unsafe { &*((p1_entry.physical_address() as usize + hhdm) as *const PageTable) };
        
        let entry = &p1.entries[virt.p1_index()];
        
        if entry.is_present() {
            Some(PhysAddr::new(entry.physical_address() + virt.page_offset() as u64))
        } else {
            None
        }
    }
    
    unsafe fn read_cr3() -> u64 {
        let cr3: u64;
        asm!("mov {}, cr3", out(reg) cr3, options(nostack, preserves_flags));
        cr3
    }
    
    pub unsafe fn load(&self) {
        let phys = self.p4_physical().as_u64();
        asm!("mov cr3, {}", in(reg) phys, options(nostack, preserves_flags));
    }
}