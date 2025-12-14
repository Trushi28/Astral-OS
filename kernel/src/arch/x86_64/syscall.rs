//! System Call Interface
//! Handles syscall/sysret for Ring 3 ↔ Ring 0 transitions

use core::arch::asm;

// MSR addresses
const IA32_STAR: u32 = 0xC0000081;
const IA32_LSTAR: u32 = 0xC0000082;
const IA32_FMASK: u32 = 0xC0000084;
const IA32_EFER: u32 = 0xC0000080;
const IA32_KERNEL_GS_BASE: u32 = 0xC0000102;

// Syscall numbers
pub const SYS_WRITE: u64 = 1;
pub const SYS_READ: u64 = 0;
pub const SYS_EXIT: u64 = 60;
pub const SYS_GETPID: u64 = 39;
pub const SYS_YIELD: u64 = 24;

/// Per-CPU data for syscall handling
#[repr(C)]
struct CpuData {
    user_rsp: u64,      // Offset 0: User stack pointer
    kernel_rsp: u64,    // Offset 8: Kernel stack pointer
}

static mut CPU_DATA: CpuData = CpuData {
    user_rsp: 0,
    kernel_rsp: 0,
};

/// Initialize syscall MSRs
pub fn init() {
    unsafe {
        // Setup kernel stack for CPU data
        static mut SYSCALL_STACK: [u8; 16384] = [0; 16384];
        let stack_top = SYSCALL_STACK.as_ptr() as u64 + 16384;
        CPU_DATA.kernel_rsp = stack_top;
        
        // Set IA32_KERNEL_GS_BASE to point to CPU_DATA
        wrmsr(IA32_KERNEL_GS_BASE, &CPU_DATA as *const _ as u64);
        
        // Enable syscall/sysret by setting EFER.SCE (bit 0)
        let efer = rdmsr(IA32_EFER);
        wrmsr(IA32_EFER, efer | 1);  // Set SCE bit
        
        // STAR: Segment selectors for syscall/sysret
        // Bits 32-47: Kernel CS (for syscall)
        // Bits 48-63: User CS (for sysret) 
        let kernel_cs = crate::arch::x86_64::gdt::KERNEL_CODE_SELECTOR as u64;
        let user_cs = crate::arch::x86_64::gdt::USER_CODE_SELECTOR as u64;
        let star = (kernel_cs << 32) | ((user_cs - 16) << 48);
        wrmsr(IA32_STAR, star);
        
        // LSTAR: Syscall entry point
        wrmsr(IA32_LSTAR, syscall_entry as *const () as u64);
        
        // FMASK: Flags to clear on syscall (disable interrupts)
        wrmsr(IA32_FMASK, 0x200);  // Clear IF
        
        crate::serial_println!("[SYSCALL] MSRs configured, EFER.SCE enabled, GS base = {:#x}", 
            &CPU_DATA as *const _ as u64);
    }
}

unsafe fn rdmsr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low,
        out("edx") high,
        options(nostack)
    );
    ((high as u64) << 32) | (low as u64)
}

unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
        options(nostack)
    );
}

/// Syscall entry point (called from Ring 3)
#[unsafe(naked)]
unsafe extern "C" fn syscall_entry() {
    core::arch::naked_asm!(
        // Save user stack
        "swapgs",
        "mov gs:[0], rsp",  // Save user RSP
        
        // Load kernel stack
        "mov rsp, gs:[8]",
        
        // Save callee-saved registers
        "push rcx",   // User RIP
        "push r11",   // User RFLAGS
        "push rbx",
        "push rbp",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        
        // Remap syscall registers to C ABI
        // Syscall uses: rax=num, rdi=arg1, rsi=arg2, rdx=arg3, r10=arg4, r8=arg5
        // C ABI uses:   rdi=p1,  rsi=p2,  rdx=p3,  rcx=p4,  r8=p5,  r9=p6
        //
        // We need: rdi=rax, rsi=rdi, rdx=rsi, rcx=rdx
        // Use r15 as scratch (already saved)
        "mov r15, rdx",   // Save arg3 (length=13)
        "mov rcx, r15",   // p4 (rcx) = arg3
        "mov r15, rsi",   // Save arg2 (buf addr)
        "mov rdx, r15",   // p3 (rdx) = arg2 (buffer)
        "mov r15, rdi",   // Save arg1 (fd=1)
        "mov rsi, r15",   // p2 (rsi) = arg1 (fd)
        "mov rdi, rax",   // p1 (rdi) = syscall number
        // r8, r9 stay same for arg5, arg6
        
        "call {handler}",
        
        // Restore registers
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbp",
        "pop rbx",
        "pop r11",   // User RFLAGS
        "pop rcx",   // User RIP
        
        // Restore user stack
        "mov rsp, gs:[0]",
        "swapgs",
        
        // Return to Ring 3
        "sysretq",
        handler = sym ring3_syscall_handler,
    );
}

#[no_mangle]
extern "C" fn ring3_syscall_handler(
    syscall_num: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    _arg4: u64,
    _arg5: u64,
) -> u64 {
    crate::serial_println!("[SYSCALL] Called: num={}, arg1={}, arg2={:#x}, arg3={}", 
        syscall_num, arg1, arg2, arg3);
    
    match syscall_num {
        SYS_WRITE => {
            crate::serial_println!("[SYSCALL] SYS_WRITE");
            // write(fd, buf, len)
            let fd = arg1;
            let buf = arg2 as *const u8;
            let len = arg3 as usize;
            
            if fd == 1 {  // stdout
                crate::serial_println!("[SYSCALL] Writing {} bytes from {:#x}", len, buf as u64);
                unsafe {
                    for i in 0..len {
                        let c = *buf.add(i);
                        crate::drivers::framebuffer::print_char(c as char, 0xFFFFFF);
                    }
                }
                len as u64
            } else {
                u64::MAX  // Error
            }
        }
        SYS_READ => {
            // read(fd, buf, len)
            let fd = arg1;
            if fd == 0 {  // stdin
                if let Some(ch) = crate::drivers::keyboard::try_read_char() {
                    ch as u64
                } else {
                    0
                }
            } else {
                u64::MAX
            }
        }
        SYS_EXIT => {
            // exit(code)
            crate::serial_println!("[SYSCALL] Process exit with code {}", arg1);
            // Return to kernel mode
            0
        }
        SYS_GETPID => {
            // Return current process ID
            1  // Placeholder
        }
        SYS_YIELD => {
            // Yield CPU
            unsafe { asm!("pause"); }
            0
        }
        _ => {
            crate::serial_println!("[SYSCALL] Unknown syscall: {}", syscall_num);
            u64::MAX
        }
    }
}
