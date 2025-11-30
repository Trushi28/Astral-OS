/// Align address up to alignment (must be power of 2)
#[inline]
pub const fn align_up(addr: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    (addr + align - 1) & !(align - 1)
}

/// Align address down to alignment (must be power of 2)
#[inline]
pub const fn align_down(addr: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    addr & !(align - 1)
}

/// Check if address is aligned
#[inline]
pub const fn is_aligned(addr: usize, align: usize) -> bool {
    debug_assert!(align.is_power_of_two());
    addr & (align - 1) == 0
}

/// Safe port I/O
#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nostack, nomem));
}

#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", out("al") val, in("dx") port, options(nostack, nomem));
    val
}

#[inline]
pub unsafe fn outw(port: u16, val: u16) {
    core::arch::asm!("out dx, ax", in("dx") port, in("ax") val, options(nostack, nomem));
}

#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let val: u16;
    core::arch::asm!("in ax, dx", out("ax") val, in("dx") port, options(nostack, nomem));
    val
}

#[inline]
pub unsafe fn outl(port: u16, val: u32) {
    core::arch::asm!("out dx, eax", in("dx") port, in("eax") val, options(nostack, nomem));
}

#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let val: u32;
    core::arch::asm!("in eax, dx", out("eax") val, in("dx") port, options(nostack, nomem));
    val
}

#[inline]
pub unsafe fn io_wait() {
    outb(0x80, 0);
}