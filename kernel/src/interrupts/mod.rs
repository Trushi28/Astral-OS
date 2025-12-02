//src/interrupts/mod.rs
pub mod idt;
pub mod pic;
pub mod handlers;

pub use idt::init_idt;
pub use idt::init_gdt_and_tss;
pub use pic::init_pic;
pub use handlers::{getchar, getchar_blocking, KB_ARROW_UP, KB_ARROW_DOWN, KB_ARROW_LEFT, KB_ARROW_RIGHT};

pub fn init() {
    crate::println!("[4/10] GDT & TSS...");
    init_gdt_and_tss();
    
    crate::println!("[5/10] IDT...");
    init_idt();
    
    crate::println!("[6/10] PIC...");
    init_pic();
    
    crate::println!("[7/10] Enabling interrupts...");
    unsafe {
        core::arch::asm!("sti", options(nostack, nomem));
    }
}
