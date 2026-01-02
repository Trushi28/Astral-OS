use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use lazy_static::lazy_static;
use crate::arch::x86_64::gdt;
use crate::serial_println;


lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        
        // Exception handlers
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);
        
        // Hardware IRQ handlers (PIC remapped to 32-47)
        idt[32].set_handler_fn(timer_interrupt_handler);    // IRQ 0 - Timer
        idt[33].set_handler_fn(keyboard_interrupt_handler); // IRQ 1 - Keyboard
        
        // Syscall interrupt (int 0x80) - for userspace syscalls
        idt[0x80].set_handler_fn(syscall_interrupt_handler);
        
        // Set default handler for all other interrupts to catch spurious IRQs
        for i in 34u8..=127u8 {
            idt[i].set_handler_fn(default_interrupt_handler);
        }
        for i in 129u8..=255u8 {
            idt[i].set_handler_fn(default_interrupt_handler);
        }
        
        idt
    };
}

pub fn init() {
    IDT.load();
    serial_println!("[IDT] Interrupt handlers configured (including int 0x80 syscall)");
}

// Hardware IRQ Handlers
extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    unsafe {
        super::timer::tick();
        
        // Scheduler tick (for preemption)
        crate::scheduler::timer_tick();
        
        // Send EOI to APIC if available, otherwise PIC
        if let Some(ref mut apic) = super::apic::local::LOCAL_APIC {
            apic.end_of_interrupt();
        } else {
            super::pic::PICS.notify_end_of_interrupt(0);
        }
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    unsafe {
        super::keyboard::handle_interrupt();
        
        // Send EOI to APIC if available, otherwise PIC
        if let Some(ref mut apic) = super::apic::local::LOCAL_APIC {
            apic.end_of_interrupt();
        } else {
            super::pic::PICS.notify_end_of_interrupt(1);
        }
    }
}

// Syscall interrupt handler (int 0x80)
extern "x86-interrupt" fn syscall_interrupt_handler(stack_frame: InterruptStackFrame) {
    // For now, just log that we received a syscall
    // Full implementation would extract registers and call syscall handler
    serial_println!("[SYSCALL] int 0x80 received from RIP: {:#x}", stack_frame.instruction_pointer.as_u64());
    
    // The actual syscall handling will be done via MSR-based SYSCALL instruction
    // This is a fallback/legacy path
}

// Exception Handlers
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

/// Enhanced page fault handler
/// 
/// Parses the error code to determine:
/// - Whether fault was from user or kernel mode
/// - Whether it was a read/write/instruction fetch violation
/// - Whether the page was present (protection) or not-present
extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    use x86_64::VirtAddr;
    
    // Cr2::read() returns Result in newer x86_64 crate versions
    let fault_addr = Cr2::read().unwrap_or(VirtAddr::zero());
    let fault_rip = stack_frame.instruction_pointer;
    
    // Parse error code flags
    let is_present = error_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION);
    let is_write = error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE);
    let is_user = error_code.contains(PageFaultErrorCode::USER_MODE);
    let is_ifetch = error_code.contains(PageFaultErrorCode::INSTRUCTION_FETCH);
    
    serial_println!("╔════════════════════════════════════════╗");
    serial_println!("║           PAGE FAULT                   ║");
    serial_println!("╠════════════════════════════════════════╣");
    serial_println!("║ Fault Address: {:#018x}     ║", fault_addr.as_u64());
    serial_println!("║ Fault RIP:     {:#018x}     ║", fault_rip.as_u64());
    serial_println!("╠════════════════════════════════════════╣");
    serial_println!("║ Mode:      {}                        ║", if is_user { "USER  " } else { "KERNEL" });
    serial_println!("║ Operation: {}                        ║", 
        if is_ifetch { "FETCH " } else if is_write { "WRITE " } else { "READ  " });
    serial_println!("║ Type:      {}            ║", 
        if is_present { "PROTECTION VIOLATION" } else { "PAGE NOT PRESENT    " });
    serial_println!("╚════════════════════════════════════════╝");
    
    // Check for stack guard page hit
    let guard_page = crate::memory::user_memory::USER_STACK_GUARD;
    if fault_addr.as_u64() >= guard_page && fault_addr.as_u64() < guard_page + 4096 {
        serial_println!("[PAGE FAULT] Stack overflow detected! Hit guard page.");
    }
    
    // Check for null pointer dereference (first 4MB reserved)
    if fault_addr.as_u64() < 0x40_0000 {
        serial_println!("[PAGE FAULT] Null pointer dereference!");
    }
    
    if is_user {
        // User mode fault - terminate the process, don't panic kernel
        serial_println!("[PAGE FAULT] User process caused fault - terminating process");
        
        // TODO: Properly terminate user process
        // For now, we still panic since we don't have full process management
        panic!("User mode page fault");
    } else {
        // Kernel mode fault - this is a serious bug
        serial_println!("{:#?}", stack_frame);
        panic!("Kernel page fault - this is a bug!");
    }
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    let is_user = (stack_frame.code_segment.0 & 3) == 3;
    
    serial_println!("╔════════════════════════════════════════╗");
    serial_println!("║      GENERAL PROTECTION FAULT          ║");
    serial_println!("╠════════════════════════════════════════╣");
    serial_println!("║ Error Code: {:#010x}                  ║", error_code);
    serial_println!("║ Mode:       {}                        ║", if is_user { "USER  " } else { "KERNEL" });
    serial_println!("║ RIP:        {:#018x}     ║", stack_frame.instruction_pointer.as_u64());
    serial_println!("╚════════════════════════════════════════╝");
    serial_println!("{:#?}", stack_frame);
    
    if is_user {
        serial_println!("[GPF] User process caused GPF - terminating process");
        panic!("User mode GPF");
    } else {
        panic!("Kernel GPF - this is a bug!");
    }
}

// Default handler for unhandled interrupts
extern "x86-interrupt" fn default_interrupt_handler(_stack_frame: InterruptStackFrame) {
    serial_println!("[WARN] Unhandled interrupt received");
    // Just return - spurious interrupt
}
