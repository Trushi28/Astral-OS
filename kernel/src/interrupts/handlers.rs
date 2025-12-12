// src/interrupts/handlers.rs (SMP-updated version)
// Key changes: Use APIC EOI instead of PIC EOI, add per-CPU tracking

use core::arch::asm;
use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
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

// Exception wrappers remain the same...
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
                "mov rdi, [rsp + 72]",
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
                "add rsp, 8",
                "iretq",
            );
        }
    };
}

// Exception handlers (unchanged)
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
    // Get RIP from stack if possible - the return address is at a known offset
    let rip: u64;
    let cs: u64;
    unsafe {
        // After the exception, the stack contains: [return addr, error_code, RIP, CS, RFLAGS, RSP, SS]
        // We need to find RIP from the interrupt frame
        asm!(
            "mov {}, [rsp + 8]",  // RIP is after error code on stack
            out(reg) rip,
            options(nostack)
        );
        asm!(
            "mov {}, [rsp + 16]",  // CS is after RIP
            out(reg) cs,
            options(nostack)
        );
    }
    
    crate::serial_println!("!!! GPF !!! error: 0x{:x}", error_code);
    crate::serial_println!("  RIP: 0x{:x}, CS: 0x{:x}", rip, cs);
    crate::serial_println!("  Halting to prevent storm...");
    
    // HALT instead of panic to prevent interrupt storm
    loop {
        unsafe { core::arch::asm!("cli; hlt"); }
    }
}


#[no_mangle]
extern "C" fn page_fault_handler(error_code: u64) {
    let cr2: u64;
    unsafe {
        asm!("mov {}, cr2", out(reg) cr2, options(nostack, nomem));
    }
    
    // Check if this is an instruction fetch fault
    let is_instruction_fetch = (error_code & 0x10) != 0;
    let is_user_mode = (error_code & 0x4) != 0;
    let is_write = (error_code & 0x2) != 0;
    let is_present = (error_code & 0x1) != 0;
    
    crate::serial_println!("!!! PAGE FAULT !!!");
    crate::serial_println!("  Address: 0x{:x}", cr2);
    crate::serial_println!("  Error: 0x{:x}", error_code);
    crate::serial_println!("  Instruction fetch: {}", is_instruction_fetch);
    crate::serial_println!("  User mode: {}", is_user_mode);
    crate::serial_println!("  Write: {}", is_write);
    crate::serial_println!("  Present: {}", is_present);
    
    panic!("Page fault");
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

// Timer interrupt (IRQ 0 / Vector 32) - NOW USES APIC EOI
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
    
    // Send APIC EOI instead of PIC EOI
    crate::arch::apic::local_apic_eoi();
    
    // Update per-CPU idle counter
    unsafe {
        let cpu_data = crate::arch::cpu::get_current_cpu_data_mut();
        cpu_data.idle_ticks += 1;
    }
    
    // Trigger scheduler (preemption)
    if let Some(next_pid) = schedule() {
        switch_to_process(next_pid);
    }
}

// Keyboard interrupt (IRQ 1 / Vector 33) - NOW USES APIC EOI
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

// Keyboard buffer and state (unchanged)
const KB_BUFFER_SIZE: usize = 256;
static mut KB_BUFFER: [u8; KB_BUFFER_SIZE] = [0; KB_BUFFER_SIZE];
static KB_WRITE_POS: AtomicUsize = AtomicUsize::new(0);
static KB_READ_POS: AtomicUsize = AtomicUsize::new(0);

pub const KB_ARROW_UP: u8 = 0x80;
pub const KB_ARROW_DOWN: u8 = 0x81;
pub const KB_ARROW_LEFT: u8 = 0x82;
pub const KB_ARROW_RIGHT: u8 = 0x83;
pub const KB_HOME: u8 = 0x84;
pub const KB_END: u8 = 0x85;
pub const KB_DELETE: u8 = 0x86;

static ESCAPE_SEQUENCE: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);

static SHIFT_PRESSED: AtomicBool = AtomicBool::new(false);
static CTRL_PRESSED: AtomicBool = AtomicBool::new(false);
static ALT_PRESSED: AtomicBool = AtomicBool::new(false);

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
        
        let esc_state = ESCAPE_SEQUENCE.load(Ordering::Relaxed);
        
        if scancode == 0xE0 {
            ESCAPE_SEQUENCE.store(1, Ordering::Relaxed);
            crate::arch::apic::local_apic_eoi(); // APIC EOI
            return;
        }
        
        if esc_state == 1 {
            ESCAPE_SEQUENCE.store(0, Ordering::Relaxed);
            
            if scancode & 0x80 == 0 {
                let special_key = match scancode {
                    0x48 => Some(KB_ARROW_UP),
                    0x50 => Some(KB_ARROW_DOWN),
                    0x4B => Some(KB_ARROW_LEFT),
                    0x4D => Some(KB_ARROW_RIGHT),
                    0x47 => Some(KB_HOME),
                    0x4F => Some(KB_END),
                    0x53 => Some(KB_DELETE),
                    _ => None,
                };
                
                if let Some(key) = special_key {
                    push_to_buffer(key);
                }
            }
            
            crate::arch::apic::local_apic_eoi(); // APIC EOI
            return;
        }
        
        if scancode & 0x80 != 0 {
            let released = scancode & 0x7F;
            match released {
                0x2A | 0x36 => SHIFT_PRESSED.store(false, Ordering::Relaxed),
                0x1D => CTRL_PRESSED.store(false, Ordering::Relaxed),
                0x38 => ALT_PRESSED.store(false, Ordering::Relaxed),
                _ => {}
            }
        } else {
            match scancode {
                0x2A | 0x36 => SHIFT_PRESSED.store(true, Ordering::Relaxed),
                0x1D => CTRL_PRESSED.store(true, Ordering::Relaxed),
                0x38 => ALT_PRESSED.store(true, Ordering::Relaxed),
                _ => {
                    let ctrl = CTRL_PRESSED.load(Ordering::Relaxed);
                    
                    if ctrl {
                        let ctrl_key = match scancode {
                            0x1E => Some(1),   // Ctrl+A
                            0x2E => Some(3),   // Ctrl+C
                            0x20 => Some(4),   // Ctrl+D
                            0x12 => Some(5),   // Ctrl+E
                            0x25 => Some(11),  // Ctrl+K
                            0x26 => Some(12),  // Ctrl+L
                            0x16 => Some(21),  // Ctrl+U
                            0x2F => Some(22),  // Ctrl+V
                            0x11 => Some(23),  // Ctrl+W
                            0x2D => Some(24),  // Ctrl+X
                            0x15 => Some(25),  // Ctrl+Y
                            0x2C => Some(26),  // Ctrl+Z
                            _ => None,
                        };
                        
                        if let Some(key) = ctrl_key {
                            push_to_buffer(key);
                        }
                    } else if (scancode as usize) < 128 {
                        let ascii = if SHIFT_PRESSED.load(Ordering::Relaxed) {
                            SCANCODE_TO_ASCII_SHIFT[scancode as usize]
                        } else {
                            SCANCODE_TO_ASCII[scancode as usize]
                        };
                        
                        if ascii != 0 {
                            push_to_buffer(ascii);
                        }
                    }
                }
            }
        }
        
        crate::arch::apic::local_apic_eoi(); // APIC EOI
    }
}

fn push_to_buffer(byte: u8) {
    unsafe {
        let write = KB_WRITE_POS.load(Ordering::Relaxed);
        let next = (write + 1) % KB_BUFFER_SIZE;
        
        if next != KB_READ_POS.load(Ordering::Relaxed) {
            KB_BUFFER[write] = byte;
            KB_WRITE_POS.store(next, Ordering::Release);
        }
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

// Syscall handler - fixed register clobbering
#[unsafe(naked)]
pub unsafe extern "C" fn syscall_wrapper() {
    core::arch::naked_asm!(
        // Save all registers
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
        
        // Save syscall args BEFORE we clobber any registers
        // Linux INT 0x80 convention: rax=syscall, rbx=arg1, rcx=arg2, rdx=arg3, rsi=arg4, rdi=arg5
        "mov r10, rdx",    // save arg3 (rdx)
        "mov r11, rsi",    // save arg4 (rsi) 
        "mov r12, rdi",    // save arg5 (rdi)
        
        // Setup System V AMD64 ABI call: rdi, rsi, rdx, rcx, r8, r9
        "mov rdi, rax",    // param1: syscall number
        "mov rsi, rbx",    // param2: arg1
        "mov rdx, rcx",    // param3: arg2
        "mov rcx, r10",    // param4: arg3 (from saved rdx)
        "mov r8, r11",     // param5: arg4 (from saved rsi)
        "mov r9, r12",     // param6: arg5 (from saved rdi)
        
        "call syscall_handler",
        
        // Store return value to stack slot for RAX (14 regs * 8 bytes from top)
        "mov [rsp + 14*8], rax",
        
        // Restore all registers
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
        "pop rax",
        
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
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
) -> u64 {
    crate::serial_println!("[SYSCALL] num={} arg1={} arg2={} arg3={}", syscall, arg1, arg2, arg3);
    
    // Record syscall for behavior tracking
    if let Some(pid) = crate::process::get_current_pid() {
        crate::security::record_syscall(pid, syscall);
    }
    
    // Dispatch to usermode syscall handler
    crate::usermode::syscall::handle_syscall(syscall, arg1, arg2, arg3, arg4, arg5)
}