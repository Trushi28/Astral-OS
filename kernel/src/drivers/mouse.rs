//! PS/2 Mouse Driver
//!
//! Handles IRQ 12 for PS/2 auxiliary device (mouse).
//! Parses 3-byte packets: [buttons, dx, dy]

use core::sync::atomic::{AtomicI32, AtomicU8, AtomicBool, Ordering};
use spin::Mutex;

// Mouse state
static MOUSE_X: AtomicI32 = AtomicI32::new(400);
static MOUSE_Y: AtomicI32 = AtomicI32::new(300);
static MOUSE_BUTTONS: AtomicU8 = AtomicU8::new(0);
static MOUSE_INITIALIZED: AtomicBool = AtomicBool::new(false);

// Screen bounds (set during init)
static SCREEN_WIDTH: AtomicI32 = AtomicI32::new(800);
static SCREEN_HEIGHT: AtomicI32 = AtomicI32::new(600);

// Packet buffer
static PACKET: Mutex<[u8; 3]> = Mutex::new([0; 3]);
static PACKET_INDEX: AtomicU8 = AtomicU8::new(0);

/// Mouse button flags
pub const BUTTON_LEFT: u8 = 0x01;
pub const BUTTON_RIGHT: u8 = 0x02;
pub const BUTTON_MIDDLE: u8 = 0x04;

/// Initialize PS/2 mouse
pub fn init() {
    // Get screen dimensions
    let (w, h) = crate::drivers::framebuffer::get_dimensions();
    SCREEN_WIDTH.store(w as i32, Ordering::Relaxed);
    SCREEN_HEIGHT.store(h as i32, Ordering::Relaxed);
    
    // Center mouse
    MOUSE_X.store(w as i32 / 2, Ordering::Relaxed);
    MOUSE_Y.store(h as i32 / 2, Ordering::Relaxed);
    
    unsafe {
        // Wait for controller ready
        wait_for_write();
        
        // Enable auxiliary device (mouse)
        crate::util::outb(0x64, 0xA8);
        wait_for_write();
        
        // Get compaq status byte
        crate::util::outb(0x64, 0x20);
        wait_for_read();
        let status = crate::util::inb(0x60);
        
        // Enable IRQ12 (bit 1) and keep mouse clock enabled
        let new_status = (status | 0x02) & !0x20;
        wait_for_write();
        crate::util::outb(0x64, 0x60);
        wait_for_write();
        crate::util::outb(0x60, new_status);
        
        // Use default settings
        write_mouse(0xF6);
        read_ack();
        
        // Enable data reporting
        write_mouse(0xF4);
        read_ack();
    }
    
    MOUSE_INITIALIZED.store(true, Ordering::Release);
    crate::serial_println!("[MOUSE] PS/2 mouse initialized");
}

/// Wait for controller input buffer empty
unsafe fn wait_for_write() {
    for _ in 0..100000 {
        if crate::util::inb(0x64) & 0x02 == 0 {
            return;
        }
    }
}

/// Wait for controller output buffer full
unsafe fn wait_for_read() {
    for _ in 0..100000 {
        if crate::util::inb(0x64) & 0x01 != 0 {
            return;
        }
    }
}

/// Write to mouse (via controller command 0xD4)
unsafe fn write_mouse(data: u8) {
    wait_for_write();
    crate::util::outb(0x64, 0xD4);
    wait_for_write();
    crate::util::outb(0x60, data);
}

/// Read ACK from mouse
unsafe fn read_ack() {
    wait_for_read();
    let _ = crate::util::inb(0x60);
}

/// Handle mouse interrupt (called from IRQ 12 handler)
pub fn handle_interrupt() {
    let byte = unsafe { crate::util::inb(0x60) };
    
    let mut packet = PACKET.lock();
    let idx = PACKET_INDEX.load(Ordering::Relaxed);
    
    // First byte must have bit 3 set (always 1)
    if idx == 0 && (byte & 0x08) == 0 {
        // Out of sync, discard
        return;
    }
    
    packet[idx as usize] = byte;
    
    if idx == 2 {
        // Complete packet - process it
        process_packet(&packet);
        PACKET_INDEX.store(0, Ordering::Relaxed);
    } else {
        PACKET_INDEX.store(idx + 1, Ordering::Relaxed);
    }
}

fn process_packet(packet: &[u8; 3]) {
    let buttons = packet[0] & 0x07;
    
    // Extract signed deltas
    let dx = if packet[0] & 0x10 != 0 {
        // Negative X
        packet[1] as i32 - 256
    } else {
        packet[1] as i32
    };
    
    let dy = if packet[0] & 0x20 != 0 {
        // Negative Y (note: PS/2 Y is inverted)
        256 - packet[2] as i32
    } else {
        -(packet[2] as i32)
    };
    
    // Update position with bounds checking
    let screen_w = SCREEN_WIDTH.load(Ordering::Relaxed);
    let screen_h = SCREEN_HEIGHT.load(Ordering::Relaxed);
    
    let new_x = (MOUSE_X.load(Ordering::Relaxed) + dx)
        .max(0)
        .min(screen_w - 1);
    let new_y = (MOUSE_Y.load(Ordering::Relaxed) + dy)
        .max(0)
        .min(screen_h - 1);
    
    MOUSE_X.store(new_x, Ordering::Relaxed);
    MOUSE_Y.store(new_y, Ordering::Relaxed);
    MOUSE_BUTTONS.store(buttons, Ordering::Relaxed);
}

/// Get current mouse position
pub fn get_position() -> (i32, i32) {
    (
        MOUSE_X.load(Ordering::Relaxed),
        MOUSE_Y.load(Ordering::Relaxed),
    )
}

/// Get current button state
pub fn get_buttons() -> u8 {
    MOUSE_BUTTONS.load(Ordering::Relaxed)
}

/// Check if left button is pressed
pub fn is_left_pressed() -> bool {
    MOUSE_BUTTONS.load(Ordering::Relaxed) & BUTTON_LEFT != 0
}

/// Check if right button is pressed
pub fn is_right_pressed() -> bool {
    MOUSE_BUTTONS.load(Ordering::Relaxed) & BUTTON_RIGHT != 0
}

/// Check if mouse is initialized
pub fn is_initialized() -> bool {
    MOUSE_INITIALIZED.load(Ordering::Acquire)
}
