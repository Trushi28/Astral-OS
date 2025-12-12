// src/usermode/loader.rs
//! ELF binary loader for user processes

use super::elf::{Elf64Header, PT_LOAD};
use crate::memory::{PageTableManager, VirtAddr, PageTableEntry};
use crate::memory::frame::allocate_frame;
use crate::util::align_up;

/// Load ELF binary into user page table
pub fn load_elf(elf_data: &[u8], page_table: &mut PageTableManager) -> Result<u64, &'static str> {
    // Parse ELF header
    let header = Elf64Header::parse(elf_data)?;
    
    // Get entry point
    let entry_point = header.entry_point();
    
    if entry_point == 0 {
        return Err("Invalid entry point");
    }
    
    // Load all PT_LOAD segments
    for ph in header.program_headers(elf_data)? {
        if ph.p_type == PT_LOAD {
            load_segment(ph, elf_data, page_table)?;
        }
    }
    
    Ok(entry_point)
}

/// Load a single ELF segment
fn load_segment(
    ph: &super::elf::Elf64ProgramHeader,
    elf_data: &[u8],
    page_table: &mut PageTableManager
) -> Result<(), &'static str> {
    let virt_addr = ph.p_vaddr;
    let mem_size = ph.p_memsz as usize;
    let file_size = ph.p_filesz as usize;
    
    if mem_size == 0 {
        return Ok(());
    }
    
    // Calculate page-aligned bounds
    let virt_start = virt_addr & !0xFFF;
    let virt_end = align_up((virt_addr + mem_size as u64) as usize, crate::PAGE_SIZE);
    let num_pages = (virt_end - virt_start as usize) / crate::PAGE_SIZE;
    
    // Determine page flags
    let mut flags = PageTableEntry::PRESENT | PageTableEntry::USER;
    
    if ph.is_writable() {
        flags |= PageTableEntry::WRITABLE;
    }
    
    if !ph.is_executable() {
        flags |= PageTableEntry::NO_EXECUTE;
    }
    
    // Allocate and map pages
    for i in 0..num_pages {
        let page_virt = VirtAddr::new(virt_start + (i * crate::PAGE_SIZE) as u64);
        let frame = allocate_frame().ok_or("Out of memory")?;
        
        // Zero the page
        unsafe {
            let page_ptr = frame.to_virt() as *mut u8;
            core::ptr::write_bytes(page_ptr, 0, crate::PAGE_SIZE);
        }
        
        page_table.map(page_virt, frame, flags)?;
    }
    
    // Copy segment data to mapped pages
    if file_size > 0 {
        let segment_data = ph.get_data(elf_data)
            .ok_or("Invalid segment offset")?;
        
        // Translate virtual to physical and copy
        for (offset, &byte) in segment_data.iter().enumerate() {
            let virt = VirtAddr::new(virt_addr + offset as u64);
            
            if let Some(phys) = page_table.translate(virt) {
                unsafe {
                    let ptr = phys.to_virt() as *mut u8;
                    *ptr = byte;
                }
            }
        }
    }
    
    Ok(())
}

/// Load a flat binary (no ELF, just raw code)
pub fn load_flat_binary(
    code: &[u8],
    page_table: &mut PageTableManager,
    load_addr: u64
) -> Result<u64, &'static str> {
    let code_size = code.len();
    let num_pages = (code_size + crate::PAGE_SIZE - 1) / crate::PAGE_SIZE;
    
    // Map pages for code
    for i in 0..num_pages {
        let page_virt = VirtAddr::new(load_addr + (i * crate::PAGE_SIZE) as u64);
        let frame = allocate_frame().ok_or("Out of memory")?;
        
        // Zero page
        unsafe {
            let page_ptr = frame.to_virt() as *mut u8;
            core::ptr::write_bytes(page_ptr, 0, crate::PAGE_SIZE);
        }
        
        page_table.map(
            page_virt,
            frame,
            PageTableEntry::PRESENT | PageTableEntry::USER | PageTableEntry::WRITABLE
        )?;
    }
    
    // Copy code
    for (offset, &byte) in code.iter().enumerate() {
        let virt = VirtAddr::new(load_addr + offset as u64);
        
        if let Some(phys) = page_table.translate(virt) {
            unsafe {
                let ptr = phys.to_virt() as *mut u8;
                *ptr = byte;
            }
        }
    }
    
    Ok(load_addr)
}