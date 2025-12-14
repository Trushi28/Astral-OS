//! Cursor Layer - Separate from main rendering
//! Allows cursor to move without redrawing entire screen

use crate::drivers::framebuffer;
use core::sync::atomic::{AtomicI32, Ordering};

static LAST_CURSOR_X: AtomicI32 = AtomicI32::new(-1);
static LAST_CURSOR_Y: AtomicI32 = AtomicI32::new(-1);
static CURSOR_BUFFER: spin::Mutex<Option<alloc::vec::Vec<u32>>> = spin::Mutex::new(None);

/// Save background under cursor
fn save_cursor_background(x: i32, y: i32) {
    // TODO: Read framebuffer pixels into buffer
    // For now, we'll just clear on next draw
}

/// Restore background under cursor
fn restore_cursor_background(x: i32, y: i32) {
    // TODO: Restore saved pixels
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
    
    // TODO: Restore old position background
    // TODO: Save new position background
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
