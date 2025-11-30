use core::arch::asm;
use super::handlers::*;

#[derive(Copy, Clone)]
#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    flags: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn new() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            flags: 0,
            offset_mid: 0,
            offset_high: 0,
            reserved: 0,
        }
    }
    
    fn set_handler(&mut self, handler: u64, ist: u8) {
        self.offset_low = handler as u16;
        self.offset_mid = (handler >> 16) as u16;
        self.offset_high = (handler >> 32) as u32;
        self.selector = 0x08; // Kernel code segment
        self.flags = 0x8E;    // Present, ring 0, interrupt gate
        self.ist = ist;
    }
    
    fn set_user_handler(&mut self, handler: u64, ist: u8) {
        self.set_handler(handler, ist);
        self.flags = 0xEE; // Present, ring 3, interrupt gate
    }
}

#[repr(C, packed)]
struct IdtDescriptor {
    limit: u16,
    base: u64,
}

const IDT_SIZE: usize = 256;
static mut IDT: [IdtEntry; IDT_SIZE] = [IdtEntry::new(); IDT_SIZE];

pub fn init_idt() {
    unsafe {
        // CPU Exceptions (0-31)
        IDT[0].set_handler(divide_error_wrapper as u64, 0);
        IDT[1].set_handler(debug_wrapper as u64, 0);
        IDT[2].set_handler(nmi_wrapper as u64, 0);
        IDT[3].set_handler(breakpoint_wrapper as u64, 0);
        IDT[4].set_handler(overflow_wrapper as u64, 0);
        IDT[5].set_handler(bound_range_wrapper as u64, 0);
        IDT[6].set_handler(invalid_opcode_wrapper as u64, 0);
        IDT[7].set_handler(device_not_available_wrapper as u64, 0);
        IDT[8].set_handler(double_fault_wrapper as u64, 1);
        IDT[10].set_handler(invalid_tss_wrapper as u64, 0);
        IDT[11].set_handler(segment_not_present_wrapper as u64, 0);
        IDT[12].set_handler(stack_segment_fault_wrapper as u64, 0);
        IDT[13].set_handler(general_protection_fault_wrapper as u64, 0);
        IDT[14].set_handler(page_fault_wrapper as u64, 0);
        IDT[16].set_handler(x87_fpu_error_wrapper as u64, 0);
        IDT[17].set_handler(alignment_check_wrapper as u64, 0);
        IDT[18].set_handler(machine_check_wrapper as u64, 0);
        IDT[19].set_handler(simd_exception_wrapper as u64, 0);
        IDT[20].set_handler(virtualization_exception_wrapper as u64, 0);
        
        // Hardware interrupts (32-47)
        IDT[32].set_handler(timer_interrupt_wrapper as u64, 0);
        IDT[33].set_handler(keyboard_interrupt_wrapper as u64, 0);
        
        // System call (0x80)
        IDT[0x80].set_user_handler(syscall_wrapper as u64, 0);
        
        let descriptor = IdtDescriptor {
            limit: (core::mem::size_of::<[IdtEntry; IDT_SIZE]>() - 1) as u16,
            base: &raw const IDT as u64,
        };
        
        asm!("lidt [{}]", in(reg) &descriptor, options(nostack));
    }
}

// GDT and TSS structures
#[repr(C, packed)]
struct Tss {
    reserved1: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved2: u64,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    reserved3: u64,
    reserved4: u16,
    iomap_base: u16,
}

static mut TSS: Tss = Tss {
    reserved1: 0, rsp0: 0, rsp1: 0, rsp2: 0, reserved2: 0,
    ist1: 0, ist2: 0, ist3: 0, ist4: 0, ist5: 0, ist6: 0, ist7: 0,
    reserved3: 0, reserved4: 0, iomap_base: 0,
};

const DOUBLE_FAULT_STACK_SIZE: usize = 4096 * 5;
static mut DOUBLE_FAULT_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0; DOUBLE_FAULT_STACK_SIZE];

#[repr(C, align(16))]
struct Gdt {
    null: u64,
    code: u64,
    data: u64,
    user_code: u64,
    user_data: u64,
    tss_low: u64,
    tss_high: u64,
}

static mut KERNEL_GDT: Gdt = Gdt {
    null: 0,
    code: 0x00AF9A000000FFFF,      // Kernel code
    data: 0x00CF92000000FFFF,      // Kernel data
    user_code: 0x00AFFA000000FFFF, // User code
    user_data: 0x00CFF2000000FFFF, // User data
    tss_low: 0,
    tss_high: 0,
};

#[repr(C, packed)]
struct GdtDescriptor {
    limit: u16,
    base: u64,
}

pub fn init_gdt_and_tss() {
    unsafe {
        TSS.ist1 = (&raw const DOUBLE_FAULT_STACK as *const _ as u64) + DOUBLE_FAULT_STACK_SIZE as u64;
        
        let tss_addr = &raw const TSS as *const _ as u64;
        let tss_limit = core::mem::size_of::<Tss>() - 1;
        
        KERNEL_GDT.tss_low = (tss_limit as u64 & 0xFFFF)
            | ((tss_addr & 0xFFFF) << 16)
            | (((tss_addr >> 16) & 0xFF) << 32)
            | (0x89u64 << 40)
            | ((tss_addr >> 24) << 56);
        KERNEL_GDT.tss_high = tss_addr >> 32;
        
        let gdt_desc = GdtDescriptor {
            limit: (core::mem::size_of::<Gdt>() - 1) as u16,
            base: &raw const KERNEL_GDT as *const _ as u64,
        };
        
        asm!("lgdt [{}]", in(reg) &gdt_desc, options(nostack));
        
        // Reload segment registers
        asm!(
            "push 0x08",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            out("rax") _,
            options(nostack)
        );
        
        // Load TSS
        asm!("ltr ax", in("ax") 0x28u16, options(nostack, nomem));
    }
}