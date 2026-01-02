use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;
use lazy_static::lazy_static;
use crate::serial_println;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

/// Kernel stack for syscall/interrupt entry from Ring 3
/// This is where RSP0 points - used when transitioning from user to kernel
static mut KERNEL_STACK: [u8; 4096 * 5] = [0; 4096 * 5];

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        
        // IST for double fault handler
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE as u64;
            stack_end
        };
        
        // RSP0 - Kernel stack for Ring 3 -> Ring 0 transitions
        // When a syscall or interrupt comes from Ring 3, CPU switches to this stack
        tss.privilege_stack_table[0] = {
            let stack_start = VirtAddr::from_ptr(unsafe { &KERNEL_STACK });
            let stack_end = stack_start + (4096 * 5) as u64;
            stack_end
        };
        
        tss
    };
}

lazy_static! {
    pub static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        
        // Index 1: Kernel code segment (selector 0x08)
        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        // Index 2: Kernel data segment (selector 0x10)
        let data_selector = gdt.append(Descriptor::kernel_data_segment());
        // Index 3: User data segment (selector 0x1B with RPL=3 -> 0x23)
        // NOTE: User data MUST come before user code for SYSRET compatibility
        let user_data_selector = gdt.append(Descriptor::user_data_segment());
        // Index 4: User code segment (selector 0x23 with RPL=3 -> 0x2B)
        let user_code_selector = gdt.append(Descriptor::user_code_segment());
        // Index 5: TSS (takes 2 entries in 64-bit mode)
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
        
        (
            gdt,
            Selectors {
                code_selector,
                data_selector,
                user_code_selector,
                user_data_selector,
                tss_selector,
            },
        )
    };
}

/// GDT segment selectors
pub struct Selectors {
    pub code_selector: SegmentSelector,
    pub data_selector: SegmentSelector,
    pub user_code_selector: SegmentSelector,
    pub user_data_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

/// Get user code segment selector (for IRETQ)
pub fn get_user_code_selector() -> SegmentSelector {
    GDT.1.user_code_selector
}

/// Get user data segment selector (for IRETQ)
pub fn get_user_data_selector() -> SegmentSelector {
    GDT.1.user_data_selector
}

/// Get kernel code segment selector
pub fn get_kernel_code_selector() -> SegmentSelector {
    GDT.1.code_selector
}

/// Get kernel data segment selector
pub fn get_kernel_data_selector() -> SegmentSelector {
    GDT.1.data_selector
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, DS, SS, Segment};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        DS::set_reg(GDT.1.data_selector);
        SS::set_reg(GDT.1.data_selector);
        load_tss(GDT.1.tss_selector);
    }
    
    serial_println!("[GDT] Loaded with Ring 3 segments:");
    serial_println!("  Kernel CS: {:#x}", GDT.1.code_selector.0);
    serial_println!("  Kernel DS: {:#x}", GDT.1.data_selector.0);
    serial_println!("  User CS:   {:#x}", GDT.1.user_code_selector.0);
    serial_println!("  User DS:   {:#x}", GDT.1.user_data_selector.0);
    serial_println!("  TSS:       {:#x}", GDT.1.tss_selector.0);
}
