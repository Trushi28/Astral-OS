pub mod gdt;
pub mod idt;
pub mod pic;
pub mod timer;
pub mod keyboard;

pub fn init() {
    gdt::init();
    idt::init();
    
    // Initialize PIC and hardware interrupts (but don't enable yet)
    unsafe {
        pic::init();
        timer::init();
        keyboard::init();
    }
    
    // Interrupts will be enabled after memory initialization
}
