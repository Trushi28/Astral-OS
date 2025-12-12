// src/acpi/mod.rs

pub mod madt;

use core::slice;

#[repr(C, packed)]
struct RsdpDescriptor {
    signature: [u8; 8],
    checksum: u8,
    oemid: [u8; 6],
    revision: u8,
    rsdt_address: u32,
}

#[repr(C, packed)]
struct RsdpDescriptor20 {
    rsdp: RsdpDescriptor,
    length: u32,
    xsdt_address: u64,
    extended_checksum: u8,
    reserved: [u8; 3],
}

#[repr(C, packed)]
pub struct SdtHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oemid: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

/// Get RSDP virtual address from Limine bootloader
/// Limine returns a HHDM-mapped virtual address, NOT physical
pub fn get_rsdp_virt_from_limine() -> Option<usize> {
    if let Some(rsdp_resp) = crate::RSDP_REQUEST.get_response() {
        // This is already a virtual address (HHDM-mapped by Limine)
        let rsdp_virt = rsdp_resp.address();
        crate::serial_println!("[ACPI] Got RSDP from Limine at virt 0x{:x}", rsdp_virt);
        Some(rsdp_virt)
    } else {
        None
    }
}

/// Legacy RSDP search (fallback, shouldn't be needed with Limine)
pub unsafe fn find_rsdp_legacy() -> Option<usize> {
    let hhdm = crate::get_hhdm_offset();
    
    // Search main BIOS area (0xE0000 - 0xFFFFF)
    let bios_start = 0xE0000 + hhdm;
    if let Some(offset) = search_rsdp_offset(bios_start, 0x20000) {
        return Some(bios_start + offset);
    }
    
    None
}

/// Search for RSDP signature and return offset from start
unsafe fn search_rsdp_offset(start: usize, length: usize) -> Option<usize> {
    let slice = slice::from_raw_parts(start as *const u8, length);
    
    for i in (0..length).step_by(16) {
        if i + 8 > length {
            break;
        }
        
        if &slice[i..i+8] == b"RSD PTR " {
            return Some(i);
        }
    }
    
    None
}

/// Get RSDP virtual address - prefer Limine's response
/// Returns virtual address (HHDM-mapped)
pub fn get_rsdp_virt() -> Option<usize> {
    // First try Limine's RSDP request (preferred for UEFI)
    if let Some(rsdp_virt) = get_rsdp_virt_from_limine() {
        return Some(rsdp_virt);
    }
    
    // Fallback to legacy BIOS search
    crate::serial_println!("[ACPI] Limine RSDP not available, searching BIOS areas...");
    unsafe { find_rsdp_legacy() }
}

pub unsafe fn parse_xsdt(xsdt_phys: u64) -> Option<&'static [u64]> {
    let hhdm = crate::get_hhdm_offset();
    let xsdt_virt = (xsdt_phys as usize + hhdm) as *const SdtHeader;
    let header = &*xsdt_virt;
    
    // Copy length field to avoid unaligned access
    let length = header.length;
    
    let entry_count = (length as usize - core::mem::size_of::<SdtHeader>()) / 8;
    let entries_ptr = xsdt_virt.add(1) as *const u64;
    
    Some(slice::from_raw_parts(entries_ptr, entry_count))
}

pub unsafe fn parse_rsdt(rsdt_phys: u32) -> Option<&'static [u32]> {
    let hhdm = crate::get_hhdm_offset();
    let rsdt_virt = (rsdt_phys as usize + hhdm) as *const SdtHeader;
    let header = &*rsdt_virt;
    
    // Copy length field to avoid unaligned access
    let length = header.length;
    
    let entry_count = (length as usize - core::mem::size_of::<SdtHeader>()) / 4;
    let entries_ptr = rsdt_virt.add(1) as *const u32;
    
    Some(slice::from_raw_parts(entries_ptr, entry_count))
}

/// Find an ACPI table by signature
/// Returns physical address of the table
pub unsafe fn find_table(signature: &[u8; 4]) -> Option<u64> {
    // Get RSDP virtual address
    let rsdp_virt = get_rsdp_virt()?;
    
    let rsdp = &*(rsdp_virt as *const RsdpDescriptor20);
    
    // Copy packed fields to local variables to avoid unaligned access
    let revision = rsdp.rsdp.revision;
    let xsdt_addr = rsdp.xsdt_address;
    let rsdt_addr = rsdp.rsdp.rsdt_address;
    
    crate::serial_println!("[ACPI] RSDP revision: {}", revision);
    
    // Try XSDT first (ACPI 2.0+)
    if revision >= 2 && xsdt_addr != 0 {
        crate::serial_println!("[ACPI] Using XSDT at phys 0x{:x}", xsdt_addr);
        if let Some(entries) = parse_xsdt(xsdt_addr) {
            for &entry_phys in entries {
                if entry_phys == 0 {
                    continue;
                }
                let hhdm = crate::get_hhdm_offset();
                let entry_virt = (entry_phys as usize + hhdm) as *const SdtHeader;
                let header = &*entry_virt;
                
                if &header.signature == signature {
                    crate::serial_println!("[ACPI] Found table '{}' at 0x{:x}", 
                        core::str::from_utf8(signature).unwrap_or("????"), entry_phys);
                    return Some(entry_phys);
                }
            }
        }
    }
    
    // Fall back to RSDT (ACPI 1.0)
    if rsdt_addr != 0 {
        crate::serial_println!("[ACPI] Using RSDT at phys 0x{:x}", rsdt_addr);
        if let Some(entries) = parse_rsdt(rsdt_addr) {
            for &entry_phys in entries {
                if entry_phys == 0 {
                    continue;
                }
                let hhdm = crate::get_hhdm_offset();
                let entry_virt = (entry_phys as usize + hhdm) as *const SdtHeader;
                let header = &*entry_virt;
                
                if &header.signature == signature {
                    return Some(entry_phys as u64);
                }
            }
        }
    }
    
    crate::serial_println!("[ACPI] Table '{}' not found", 
        core::str::from_utf8(signature).unwrap_or("????"));
    None
}