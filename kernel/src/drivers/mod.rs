//src/drivers/mod.rs
pub mod serial;
pub mod keyboard;
pub mod framebuffer;
pub mod virtio;
pub mod mouse;

pub fn init() {
    crate::println!("[8/10] Serial port...");
    serial::init();
    
    crate::println!("[9/10] VirtIO disk...");
    virtio::init();
    
    crate::println!("[10/10] PS/2 Mouse...");
    mouse::init();
}