//src/drivers/mod.rs
pub mod serial;
pub mod keyboard;
pub mod framebuffer;
pub mod virtio;

pub fn init() {
    crate::println!("[8/10] Serial port...");
    serial::init();
    
    crate::println!("[9/10] VirtIO disk...");
    virtio::init();
}