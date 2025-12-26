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
        
        // Set default handler for all other interrupts to catch spurious IRQs
        for i in 34u8..=255u8 {
            idt[i].set_handler_fn(default_interrupt_handler);
        }
        
        idt
    };
}

pub fn init() {
    IDT.load();
}

// Hardware IRQ Handlers
extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    unsafe {
        super::timer::tick();
        super::pic::PICS.notify_end_of_interrupt(0);
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    unsafe {
        super::keyboard::handle_interrupt();
        super::pic::PICS.notify_end_of_interrupt(1);
    }
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

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    serial_println!("EXCEPTION: PAGE FAULT");
    serial_println!("Accessed Address: {:?}", x86_64::registers::control::Cr2::read());
    serial_println!("Error Code: {:?}", error_code);
    serial_println!("{:#?}", stack_frame);
    
    panic!("Page fault");
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    serial_println!("EXCEPTION: GENERAL PROTECTION FAULT");
    serial_println!("Error Code: {}", error_code);
    serial_println!("{:#?}", stack_frame);
    
    panic!("General protection fault");
}

// Default handler for unhandled interrupts
extern "x86-interrupt" fn default_interrupt_handler(_stack_frame: InterruptStackFrame) {
    serial_println!("[WARN] Unhandled interrupt received");
    // Just return - spurious interrupt
}
