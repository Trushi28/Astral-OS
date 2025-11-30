//src/interrupts/handlers.rs
use core::arch::asm;
use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use super::pic::pic_send_eoi;
use crate::process::scheduler::schedule;
use crate::process::context::switch_to_process;

#[repr(C)]
pub struct InterruptFrame {
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

// Exception wrappers that preserve all registers
macro_rules! exception_wrapper {
    ($name:ident, $handler:ident) => {
        #[unsafe(naked)]
        pub unsafe extern "C" fn $name() {
            core::arch::naked_asm!(
                "push rax",
                "push rcx",
                "push rdx",
                "push rsi",
                "push rdi",
                "push r8",
                "push r9",
                "push r10",
                "push r11",
                concat!("call ", stringify!($handler)),
                "pop r11",
                "pop r10",
                "pop r9",
                "pop r8",
                "pop rdi",
                "pop rsi",
                "pop rdx",
                "pop rcx",
                "pop rax",
                "iretq",
            );
        }
    };
}

macro_rules! exception_with_error_wrapper {
    ($name:ident, $handler:ident) => {
        #[unsafe(naked)]
        pub unsafe extern "C" fn $name() {
            core::arch::naked_asm!(
                "push rax",
                "push rcx",
                "push rdx",
                "push rsi",
                "push rdi",
                "push r8",
                "push r9",
                "push r10",
                "push r11",
                "mov rdi, [rsp + 72]", // Error code is at rsp+72 (9*8)
                concat!("call ", stringify!($handler)),
                "pop r11",
                "pop r10",
                "pop r9",
                "pop r8",
                "pop rdi",
                "pop rsi",
                "pop rdx",
                "pop rcx",
                "pop rax",
                "add rsp, 8", // Remove error code
                "iretq",
            );
        }
    };
}

// Exception handlers
exception_wrapper!(divide_error_wrapper, divide_error_handler);
exception_wrapper!(debug_wrapper, debug_handler);
exception_wrapper!(nmi_wrapper, nmi_handler);
exception_wrapper!(breakpoint_wrapper, breakpoint_handler);
exception_wrapper!(overflow_wrapper, overflow_handler);
exception_wrapper!(bound_range_wrapper, bound_range_handler);
exception_wrapper!(invalid_opcode_wrapper, invalid_opcode_handler);
exception_wrapper!(device_not_available_wrapper, device_not_available_handler);
exception_wrapper!(x87_fpu_error_wrapper, x87_fpu_error_handler);
exception_wrapper!(machine_check_wrapper, machine_check_handler);
exception_wrapper!(simd_exception_wrapper, simd_exception_handler);
exception_wrapper!(virtualization_exception_wrapper, virtualization_exception_handler);

exception_with_error_wrapper!(double_fault_wrapper, double_fault_handler);
exception_with_error_wrapper!(invalid_tss_wrapper, invalid_tss_handler);
exception_with_error_wrapper!(segment_not_present_wrapper, segment_not_present_handler);
exception_with_error_wrapper!(stack_segment_fault_wrapper, stack_segment_fault_handler);
exception_with_error_wrapper!(general_protection_fault_wrapper, general_protection_fault_handler);
exception_with_error_wrapper!(page_fault_wrapper, page_fault_handler);
exception_with_error_wrapper!(alignment_check_wrapper, alignment_check_handler);

#[no_mangle]
extern "C" fn divide_error_handler() {
    panic!("EXCEPTION: Divide by Zero");
}

#[no_mangle]
extern "C" fn debug_handler() {
    crate::println!("DEBUG: Debug exception");
}

#[no_mangle]
extern "C" fn nmi_handler() {
    panic!("EXCEPTION: Non-Maskable Interrupt");
}

#[no_mangle]
extern "C" fn breakpoint_handler() {
    crate::println!("DEBUG: Breakpoint");
}

#[no_mangle]
extern "C" fn overflow_handler() {
    panic!("EXCEPTION: Overflow");
}

#[no_mangle]
extern "C" fn bound_range_handler() {
    panic!("EXCEPTION: Bound Range Exceeded");
}

#[no_mangle]
extern "C" fn invalid_opcode_handler() {
    panic!("EXCEPTION: Invalid Opcode");
}

#[no_mangle]
extern "C" fn device_not_available_handler() {
    panic!("EXCEPTION: Device Not Available");
}

#[no_mangle]
extern "C" fn double_fault_handler(error_code: u64) -> ! {
    panic!("EXCEPTION: Double Fault (error: 0x{:x})", error_code);
}

#[no_mangle]
extern "C" fn invalid_tss_handler(error_code: u64) {
    panic!("EXCEPTION: Invalid TSS (error: 0x{:x})", error_code);
}

#[no_mangle]
extern "C" fn segment_not_present_handler(error_code: u64) {
    panic!("EXCEPTION: Segment Not Present (error: 0x{:x})", error_code);
}

#[no_mangle]
extern "C" fn stack_segment_fault_handler(error_code: u64) {
    panic!("EXCEPTION: Stack Segment Fault (error: 0x{:x})", error_code);
}

#[no_mangle]
extern "C" fn general_protection_fault_handler(error_code: u64) {
    panic!("EXCEPTION: General Protection Fault (error: 0x{:x})", error_code);
}

#[no_mangle]
extern "C" fn page_fault_handler(error_code: u64) {
    let cr2: u64;
    unsafe {
        asm!("mov {}, cr2", out(reg) cr2, options(nostack, nomem));
    }
    
    panic!(
        "EXCEPTION: Page Fault\n  Address: 0x{:x}\n  Error: 0x{:x}",
        cr2, error_code
    );
}

#[no_mangle]
extern "C" fn x87_fpu_error_handler() {
    panic!("EXCEPTION: x87 FPU Error");
}

#[no_mangle]
extern "C" fn alignment_check_handler(error_code: u64) {
    panic!("EXCEPTION: Alignment Check (error: 0x{:x})", error_code);
}

#[no_mangle]
extern "C" fn machine_check_handler() -> ! {
    panic!("EXCEPTION: Machine Check");
}

#[no_mangle]
extern "C" fn simd_exception_handler() {
    panic!("EXCEPTION: SIMD Floating-Point Exception");
}

#[no_mangle]
extern "C" fn virtualization_exception_handler() {
    panic!("EXCEPTION: Virtualization Exception");
}

// Timer interrupt (IRQ 0)
#[unsafe(naked)]
pub unsafe extern "C" fn timer_interrupt_wrapper() {
    core::arch::naked_asm!(
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "call timer_interrupt_handler",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
    );
}

#[no_mangle]
extern "C" fn timer_interrupt_handler() {
    crate::increment_timestamp();
    
    unsafe {
        pic_send_eoi(0);
    }
    
    // Trigger scheduler
    if let Some(next_pid) = schedule() {
        switch_to_process(next_pid);
    }
}

// Keyboard interrupt (IRQ 1)
#[unsafe(naked)]
pub unsafe extern "C" fn keyboard_interrupt_wrapper() {
    core::arch::naked_asm!(
        "push rax",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "call keyboard_interrupt_handler",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rax",
        "iretq",
    );
}

// Keyboard buffer
const KB_BUFFER_SIZE: usize = 256;
static mut KB_BUFFER: [u8; KB_BUFFER_SIZE] = [0; KB_BUFFER_SIZE];
static KB_WRITE_POS: AtomicUsize = AtomicUsize::new(0);
static KB_READ_POS: AtomicUsize = AtomicUsize::new(0);

static SHIFT_PRESSED: AtomicBool = AtomicBool::new(false);
static CTRL_PRESSED: AtomicBool = AtomicBool::new(false);

// US keyboard scancode map
static SCANCODE_TO_ASCII: [u8; 128] = [
    0, 27, b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0', b'-', b'=', 8,
    b'\t', b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i', b'o', b'p', b'[', b']', b'\n',
    0, b'a', b's', b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';', b'\'', b'`',
    0, b'\\', b'z', b'x', b'c', b'v', b'b', b'n', b'm', b',', b'.', b'/', 0,
    b'*', 0, b' ', 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0,
    b'7', b'8', b'9', b'-', b'4', b'5', b'6', b'+', b'1', b'2', b'3', b'0', b'.',
    0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
];

static SCANCODE_TO_ASCII_SHIFT: [u8; 128] = [
    0, 27, b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')', b'_', b'+', 8,
    b'\t', b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P', b'{', b'}', b'\n',
    0, b'A', b'S', b'D', b'F', b'G', b'H', b'J', b'K', b'L', b':', b'"', b'~',
    0, b'|', b'Z', b'X', b'C', b'V', b'B', b'N', b'M', b'<', b'>', b'?', 0,
    b'*', 0, b' ', 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0,
    b'7', b'8', b'9', b'-', b'4', b'5', b'6', b'+', b'1', b'2', b'3', b'0', b'.',
    0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0,
];

#[no_mangle]
extern "C" fn keyboard_interrupt_handler() {
    unsafe {
        let scancode = crate::util::inb(0x60);
        
        // Handle key release
        if scancode & 0x80 != 0 {
            let released = scancode & 0x7F;
            match released {
                0x2A | 0x36 => SHIFT_PRESSED.store(false, Ordering::Relaxed),
                0x1D => CTRL_PRESSED.store(false, Ordering::Relaxed),
                _ => {}
            }
        } else {
            // Handle key press
            match scancode {
                0x2A | 0x36 => SHIFT_PRESSED.store(true, Ordering::Relaxed),
                0x1D => CTRL_PRESSED.store(true, Ordering::Relaxed),
                _ => {
                    if (scancode as usize) < 128 {
                        let ascii = if SHIFT_PRESSED.load(Ordering::Relaxed) {
                            SCANCODE_TO_ASCII_SHIFT[scancode as usize]
                        } else {
                            SCANCODE_TO_ASCII[scancode as usize]
                        };
                        
                        if ascii != 0 {
                            let write = KB_WRITE_POS.load(Ordering::Relaxed);
                            let next = (write + 1) % KB_BUFFER_SIZE;
                            
                            if next != KB_READ_POS.load(Ordering::Relaxed) {
                                KB_BUFFER[write] = ascii;
                                KB_WRITE_POS.store(next, Ordering::Release);
                            }
                        }
                    }
                }
            }
        }
        
        pic_send_eoi(1);
    }
}

pub fn getchar() -> Option<u8> {
    let read = KB_READ_POS.load(Ordering::Relaxed);
    let write = KB_WRITE_POS.load(Ordering::Acquire);
    
    if read == write {
        return None;
    }
    
    let c = unsafe { KB_BUFFER[read] };
    KB_READ_POS.store((read + 1) % KB_BUFFER_SIZE, Ordering::Release);
    Some(c)
}

pub fn getchar_blocking() -> u8 {
    loop {
        if let Some(c) = getchar() {
            return c;
        }
        unsafe { asm!("hlt"); }
    }
}

// Syscall handler
#[unsafe(naked)]
pub unsafe extern "C" fn syscall_wrapper() {
    core::arch::naked_asm!(
        "push rax",
        "push rbx",
        "push rcx",
        "push rdx",
        "push rsi",
        "push rdi",
        "push rbp",
        "push r8",
        "push r9",
        "push r10",
        "push r11",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov rdi, rax",
        "mov rsi, rbx",
        "mov rdx, rcx",
        "mov rcx, rdx",
        "call syscall_handler",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop r11",
        "pop r10",
        "pop r9",
        "pop r8",
        "pop rbp",
        "pop rdi",
        "pop rsi",
        "pop rdx",
        "pop rcx",
        "pop rbx",
        "add rsp, 8",
        "iretq",
    );
}

const SYS_EXIT: u64 = 0;
const SYS_WRITE: u64 = 2;
const SYS_GETPID: u64 = 7;
const SYS_YIELD: u64 = 9;

#[no_mangle]
extern "C" fn syscall_handler(
    syscall: u64,
    _arg1: u64,
    _arg2: u64,
    _arg3: u64,
) -> u64 {
    match syscall {
        SYS_EXIT => {
            crate::process::exit_process(0);
            0
        }
        SYS_WRITE => {
            // TODO: Implement
            0
        }
        SYS_GETPID => {
            crate::process::get_current_pid().map(|p| p.as_u64()).unwrap_or(0)
        }
        SYS_YIELD => {
            crate::process::scheduler::yield_cpu();
            0
        }
        _ => {
            crate::println!("Unknown syscall: {}", syscall);
            !0u64
        }
    }
}