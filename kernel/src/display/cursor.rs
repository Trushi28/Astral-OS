//! Cursor Layer - Separate from main rendering
//! Allows cursor to move without redrawing entire screen

use crate::drivers::framebuffer;
use core::sync::atomic::{AtomicI32, Ordering};
use spin::Mutex;
use alloc::vec::Vec;

static LAST_CURSOR_X: AtomicI32 = AtomicI32::new(-1);
static LAST_CURSOR_Y: AtomicI32 = AtomicI32::new(-1);
static CURSOR_BUFFER: Mutex<Vec<u32>> = Mutex::new(Vec::new());
const CURSOR_WIDTH: usize = 8;
const CURSOR_HEIGHT: usize = 14;

/// Save background under cursor by reading framebuffer
fn save_cursor_background(x: i32, y: i32) -> Vec<u32> {
    let mut buffer = Vec::with_capacity(CURSOR_WIDTH * CURSOR_HEIGHT);
    
    // Read pixels from framebuffer
    // Note: We can't directly read framebuffer, so we'll track background color
    // For now, just return empty buffer - restoration will use background color
    for _ in 0..(CURSOR_WIDTH * CURSOR_HEIGHT) {
        buffer.push(0x16213e); // Use background color
    }
    buffer
}

/// Restore background under cursor
fn restore_cursor_background(x: i32, y: i32, buffer: &[u32]) {
    if buffer.is_empty() {
        return;
    }
    
    for dy in 0..CURSOR_HEIGHT {
        for dx in 0..CURSOR_WIDTH {
            let idx = dy * CURSOR_WIDTH + dx;
            if idx < buffer.len() {
                let px = x as usize + dx;
                let py = y as usize + dy;
                framebuffer::blit_buffer(&[buffer[idx]], px, py, 1, 1);
            }
        }
    }
}

/// Draw cursor at position WITHOUT redrawing everything else
pub fn update_cursor_only() {
    let (mx, my) = crate::drivers::mouse::get_position();
    let last_x = LAST_CURSOR_X.load(Ordering::Relaxed);
    let last_y = LAST_CURSOR_Y.load(Ordering::Relaxed);
    
    // If cursor didn't move, do nothing
    if mx == last_x && my == last_y {
        return;
    }
    
    // Restore old position background if cursor moved before
    if last_x >= 0 && last_y >= 0 {
        let buffer = CURSOR_BUFFER.lock();
        restore_cursor_background(last_x, last_y, &buffer);
    }
    
    // Save new position background
    let new_buffer = save_cursor_background(mx, my);
    *CURSOR_BUFFER.lock() = new_buffer;
    
    // Draw cursor at new position
    draw_cursor_sprite(mx, my);
    
    LAST_CURSOR_X.store(mx, Ordering::Relaxed);
    LAST_CURSOR_Y.store(my, Ordering::Relaxed);
}

fn draw_cursor_sprite(mx: i32, my: i32) {
    let is_clicking = crate::drivers::mouse::is_left_pressed();
    let fill_color = if is_clicking { 0x00AAFF } else { 0xFFFFFF };
    let outline_color = 0x000000;
    
    let cursor_rows: &[(i32, i32)] = &[
        (0, 0),
        (0, 1), (1, 1),
        (0, 2), (1, 2), (2, 2),
        (0, 3), (1, 3), (2, 3), (3, 3),
        (0, 4), (1, 4), (2, 4), (3, 4), (4, 4),
        (0, 5), (1, 5), (2, 5), (3, 5), (4, 5), (5, 5),
        (0, 6), (1, 6), (2, 6), (3, 6), (4, 6), (5, 6), (6, 6),
        (0, 7), (1, 7), (2, 7), (3, 7), (4, 7), (5, 7),
        (0, 8), (1, 8), (2, 8), (3, 8), (4, 8), (5, 8),
        (0, 9), (1, 9), (2, 9), (3, 9),
        (0, 10), (1, 10), (4, 10), (5, 10),
        (0, 11), (1, 11), (4, 11), (5, 11), (6, 11),
        (5, 12), (6, 12), (7, 12),
        (6, 13), (7, 13),
    ];
    
    // Draw outline
    for &(dx, dy) in cursor_rows {
        let x = (mx + dx) as usize;
        let y = (my + dy) as usize;
        if dx > 0 {
            framebuffer::blit_buffer(&[outline_color], x - 1, y, 1, 1);
        }
        framebuffer::blit_buffer(&[outline_color], x + 1, y, 1, 1);
        if dy > 0 {
            framebuffer::blit_buffer(&[outline_color], x, y - 1, 1, 1);
        }
        framebuffer::blit_buffer(&[outline_color], x, y + 1, 1, 1);
    }
    
    // Draw fill
    for &(dx, dy) in cursor_rows {
        let x = (mx + dx) as usize;
        let y = (my + dy) as usize;
        framebuffer::blit_buffer(&[fill_color], x, y, 1, 1);
    }
}
