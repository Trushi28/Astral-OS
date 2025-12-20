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
pub const SYS_GETUID: u64 = 102;
pub const SYS_YIELD_ALT: u64 = 158;  // Linux sched_yield syscall number

// Shell-specific syscalls
pub const SYS_FS_LIST: u64 = 200;
pub const SYS_GET_PROCESS_INFO: u64 = 201;
pub const SYS_GET_MEM_INFO: u64 = 202;
pub const SYS_GET_CPU_INFO: u64 = 203;
pub const SYS_CLEAR_SCREEN: u64 = 204;
pub const SYS_GET_TIME: u64 = 205;
pub const SYS_GET_UPTIME: u64 = 206;

use core::sync::atomic::{AtomicBool, Ordering};

/// Flag to indicate user shell has exited and should return to menu
pub static SHELL_EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Check if shell requested exit
pub fn shell_exit_requested() -> bool {
    SHELL_EXIT_REQUESTED.load(Ordering::SeqCst)
}

/// Clear the exit flag
pub fn clear_shell_exit() {
    SHELL_EXIT_REQUESTED.store(false, Ordering::SeqCst);
}

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
        // Setup kernel stack for CPU data using raw pointers
        static mut SYSCALL_STACK: [u8; 16384] = [0; 16384];
        let stack_ptr = &raw const SYSCALL_STACK;
        let stack_top = (*stack_ptr).as_ptr() as u64 + 16384;
        
        let cpu_data_ptr = &raw mut CPU_DATA;
        (*cpu_data_ptr).kernel_rsp = stack_top;
        
        // Set IA32_KERNEL_GS_BASE to point to CPU_DATA (raw pointer)
        wrmsr(IA32_KERNEL_GS_BASE, (&raw const CPU_DATA) as u64);
        
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
            (&raw const CPU_DATA) as u64);
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
    // Handle syscalls silently (removed verbose logging)
    match syscall_num {
        SYS_WRITE => {
            // write(fd, buf, len)
            let fd = arg1;
            let buf = arg2;  // User virtual address
            let len = arg3 as usize;
            
            if fd == 1 && len > 0 && buf != 0 {  // stdout, valid length, non-null buffer
                // Write ALL bytes from the buffer
                unsafe {
                    for i in 0..len {
                        let user_ptr = (buf + i as u64) as *const u8;
                        let c = core::ptr::read_volatile(user_ptr);
                        crate::drivers::framebuffer::print_char(c as char, 0xFFFFFF);
                    }
                }
                len as u64  // Return number of bytes written
            } else {
                u64::MAX  // Error
            }
        }
        SYS_READ => {
            // read(fd, buf, len) - returns bytes read
            let fd = arg1;
            let buf = arg2 as *mut u8;
            let _len = arg3 as usize;
            
            if fd == 0 && !buf.is_null() {  // stdin
                if let Some(ch) = crate::drivers::keyboard::try_read_char() {
                    unsafe {
                        *buf = ch as u8;
                    }
                    1  // Successfully read 1 byte
                } else {
                    0  // No input available
                }
            } else {
                u64::MAX  // Error
            }
        }
        SYS_EXIT => {
            // exit(code) - Signal shell exit and halt cleanly
            crate::serial_println!("[SYSCALL] Process exit with code {}", arg1);
            
            // Set exit flag so kernel knows to return to menu
            SHELL_EXIT_REQUESTED.store(true, Ordering::SeqCst);
            
            // Mark current process as zombie
            crate::process::exit_process(arg1 as i32);
            
            // Clear the screen and show message
            crate::drivers::framebuffer::clear();
            crate::drivers::framebuffer::print_colored(
                "\n\n  Shell exited. Press any key to return to menu...\n", 
                0x00AAFF
            );
            
            crate::serial_println!("[SYSCALL] Shell exit complete, waiting for keypress...");
            
            // Wait for a keypress before returning
            loop {
                if crate::drivers::keyboard::try_read_char().is_some() {
                    break;
                }
                // Enable interrupts and halt until interrupt
                unsafe { asm!("sti; hlt"); }
            }
            
            crate::drivers::framebuffer::clear();
            
            // Now we need to return to the kernel's boot menu
            // Since sysretq would return to RIP=0 (bad), we instead
            // directly jump to a safe kernel function
            
            // The simplest fix: just loop here and let the user reboot
            // A proper fix would involve longjmp-style return
            crate::serial_println!("[SYSCALL] Returning to boot menu...");
            
            // Jump to kernel's main loop by using a software reset approach
            // We'll just halt and require reboot for now, with a message
            crate::drivers::framebuffer::print_colored(
                "\n\n  Session ended. System halted.\n  Press Ctrl-A X (QEMU) or reset to restart.\n",
                0xFFAA00
            );
            
            loop {
                unsafe { asm!("cli; hlt"); }
            }
        }
        SYS_GETPID => {
            // Return current process ID
            if let Some(pid) = crate::process::get_current_pid() {
                pid.as_u64()
            } else {
                1  // Fallback
            }
        }
        SYS_GETUID => {
            // Return current user ID from process
            if let Some(pid) = crate::process::get_current_pid() {
                let table = crate::process::process_table().lock();
                if let Some(proc) = table.get(pid) {
                    proc.uid as u64
                } else {
                    0  // Default UID
                }
            } else {
                0  // Default UID
            }
        }
        SYS_YIELD | SYS_YIELD_ALT => {
            // Yield CPU
            unsafe { asm!("pause"); }
            0
        }
        
        // Shell-specific syscalls
        SYS_FS_LIST => {
            // Returns number of files, writes names to serial for now
            let files = crate::fs::psychicfs::fs_list();
            crate::serial_println!("[SYSCALL] SYS_FS_LIST: {} files", files.len());
            // Write file list to framebuffer directly (kernel handles it)
            if files.is_empty() {
                crate::drivers::framebuffer::print_colored("(empty)\n", 0xFFFFFF);
            } else {
                for file in &files {
                    crate::drivers::framebuffer::print_colored(file, 0x88FF88);
                    crate::drivers::framebuffer::print_colored("  ", 0xFFFFFF);
                }
                crate::drivers::framebuffer::print_colored("\n", 0xFFFFFF);
            }
            files.len() as u64
        }
        
        SYS_GET_PROCESS_INFO => {
            // Get process info and print it
            let table = crate::process::process_table().lock();
            crate::drivers::framebuffer::print_colored("PID  STATE     NAME\n", 0x88FFFF);
            crate::drivers::framebuffer::print_colored("---  --------  ----\n", 0x888888);
            
            for proc in table.iter() {
                let state = match proc.state {
                    crate::process::ProcessState::Ready => "READY   ",
                    crate::process::ProcessState::Running => "RUNNING ",
                    crate::process::ProcessState::Blocked => "BLOCKED ",
                    crate::process::ProcessState::Zombie => "ZOMBIE  ",
                };
                // Print PID and state
                let pid = proc.pid.as_u64();
                if pid >= 10 { 
                    crate::drivers::framebuffer::print_char((b'0' + (pid / 10 % 10) as u8) as char, 0xFFFFFF);
                }
                crate::drivers::framebuffer::print_char((b'0' + (pid % 10) as u8) as char, 0xFFFFFF);
                crate::drivers::framebuffer::print_colored("    ", 0xFFFFFF);
                crate::drivers::framebuffer::print_colored(state, 0x88FF88);
                crate::drivers::framebuffer::print_colored("\n", 0xFFFFFF);
            }
            
            crate::drivers::framebuffer::print_colored("\nTotal: ", 0xFFFFFF);
            let count = table.count();
            if count >= 10 {
                crate::drivers::framebuffer::print_char((b'0' + (count / 10 % 10) as u8) as char, 0xFFFFFF);
            }
            crate::drivers::framebuffer::print_char((b'0' + (count % 10) as u8) as char, 0xFFFFFF);
            crate::drivers::framebuffer::print_colored(" processes\n", 0xFFFFFF);
            
            table.count() as u64
        }
        
        SYS_GET_MEM_INFO => {
            // Get memory stats and print them
            let (total, used, free) = crate::memory::frame::get_stats();
            crate::drivers::framebuffer::print_colored("Physical Memory:\n", 0x88FFFF);
            crate::drivers::framebuffer::print_colored("  Total: ", 0xFFFFFF);
            print_number_to_fb((total * 4 / 1024) as u64);
            crate::drivers::framebuffer::print_colored(" MB\n", 0xFFFFFF);
            crate::drivers::framebuffer::print_colored("  Free:  ", 0xFFFFFF);
            print_number_to_fb((free * 4 / 1024) as u64);
            crate::drivers::framebuffer::print_colored(" MB\n", 0xFFFFFF);
            
            // Return packed value
            ((total as u64) << 32) | (free as u64)
        }
        
        SYS_GET_CPU_INFO => {
            // Get CPU info and print it
            let cpu_count = crate::arch::x86_64::cpu::get_cpu_count();
            crate::drivers::framebuffer::print_colored("CPU Information:\n", 0x88FFFF);
            crate::drivers::framebuffer::print_colored("  Architecture: x86_64\n", 0xFFFFFF);
            crate::drivers::framebuffer::print_colored("  Cores: ", 0xFFFFFF);
            print_number_to_fb(cpu_count as u64);
            crate::drivers::framebuffer::print_colored("\n", 0xFFFFFF);
            crate::drivers::framebuffer::print_colored("  Mode: Long Mode (64-bit)\n", 0xFFFFFF);
            
            cpu_count as u64
        }
        
        SYS_CLEAR_SCREEN => {
            crate::drivers::framebuffer::clear();
            0
        }
        
        SYS_GET_TIME => {
            // Get system time (uptime in ticks)
            let ticks = crate::get_timestamp();
            ticks
        }
        
        SYS_GET_UPTIME => {
            // Get uptime in seconds (assuming 100Hz timer)
            let ticks = crate::get_timestamp();
            let secs = ticks / 100;
            crate::drivers::framebuffer::print_colored("Uptime: ", 0x88FFFF);
            print_number_to_fb(secs);
            crate::drivers::framebuffer::print_colored(" seconds\n", 0xFFFFFF);
            secs
        }
        
        _ => {
            crate::serial_println!("[SYSCALL] Unknown syscall: {}", syscall_num);
            u64::MAX
        }
    }
}

/// Helper to print a number to framebuffer
fn print_number_to_fb(n: u64) {
    if n >= 10 {
        print_number_to_fb(n / 10);
    }
    let digit = (n % 10) as u8 + b'0';
    crate::drivers::framebuffer::print_char(digit as char, 0xFFFFFF);
}
