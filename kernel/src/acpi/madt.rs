// src/acpi/madt.rs

use super::SdtHeader;
use alloc::vec::Vec;

#[repr(C, packed)]
pub struct Madt {
    pub header: SdtHeader,
    pub local_apic_address: u32,
    pub flags: u32,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum MadtEntryType {
    LocalApic = 0,
    IoApic = 1,
    InterruptSourceOverride = 2,
    LocalApicNmi = 4,
    LocalApicAddressOverride = 5,
    X2Apic = 9,
}

#[repr(C, packed)]
pub struct MadtEntryHeader {
    pub entry_type: u8,
    pub length: u8,
}

#[repr(C, packed)]
pub struct LocalApicEntry {
    pub header: MadtEntryHeader,
    pub acpi_processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

#[repr(C, packed)]
pub struct IoApicEntry {
    pub header: MadtEntryHeader,
    pub io_apic_id: u8,
    pub reserved: u8,
    pub io_apic_address: u32,
    pub global_system_interrupt_base: u32,
}

#[repr(C, packed)]
pub struct X2ApicEntry {
    pub header: MadtEntryHeader,
    pub reserved: u16,
    pub x2apic_id: u32,
    pub flags: u32,
    pub acpi_processor_uid: u32,
}

pub struct MadtInfo {
    pub local_apic_addr: u64,
    pub local_apics: Vec<u32>,
    pub ioapic_addr: u64,
    pub ioapic_gsi_base: u32,
}

pub unsafe fn parse_madt() -> Option<MadtInfo> {
    let madt_phys = super::find_table(b"APIC")?;
    let madt_virt = (madt_phys as usize + crate::get_hhdm_offset()) as *const Madt;
    let madt = &*madt_virt;
    
    let mut info = MadtInfo {
        local_apic_addr: madt.local_apic_address as u64,
        local_apics: Vec::new(),
        ioapic_addr: 0,
        ioapic_gsi_base: 0,
    };
    
    let entries_start = madt_virt.add(1) as usize;
    let entries_end = madt_virt as usize + madt.header.length as usize;
    
    let mut offset = entries_start;
    while offset < entries_end {
        let header = &*(offset as *const MadtEntryHeader);
        
        match header.entry_type {
            0 => { // Local APIC
                let entry = &*(offset as *const LocalApicEntry);
                if (entry.flags & 1) != 0 { // Enabled
                    info.local_apics.push(entry.apic_id as u32);
                }
            }
            1 => { // I/O APIC
                let entry = &*(offset as *const IoApicEntry);
                info.ioapic_addr = entry.io_apic_address as u64;
                info.ioapic_gsi_base = entry.global_system_interrupt_base;
            }
            9 => { // x2APIC
                let entry = &*(offset as *const X2ApicEntry);
                if (entry.flags & 1) != 0 {
                    info.local_apics.push(entry.x2apic_id);
                }
            }
            _ => {}
        }
        
        offset += header.length as usize;
    }
    
    Some(info)
}