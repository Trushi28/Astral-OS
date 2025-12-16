//! Embedded Ring 3 Shell Binary
//! Position-independent machine code for a user-mode shell
//!
//! Uses Linux x86-64 syscall ABI:
//! - rax = syscall number
//! - rdi = arg1, rsi = arg2, rdx = arg3
//! - syscall clobbers rcx, r11, and rax (return value)

/// Syscall numbers
const SYS_WRITE: u8 = 1;
const SYS_READ: u8 = 0;
const SYS_EXIT: u8 = 60;

/// Get the embedded shell binary
pub fn get_ring3_shell_binary() -> alloc::vec::Vec<u8> {
    build_ring3_shell()
}

/// Build a minimal Ring 3 shell
pub fn build_ring3_shell() -> alloc::vec::Vec<u8> {
    let mut code = alloc::vec::Vec::new();
    
    // Data section at offset 256
    let banner = b"\n=== Ring 3 User Shell ===\nType: help, ls, ps, mem, exit\n\n";
    let prompt = b"user> ";
    let data_offset: i32 = 256;
    
    // ===== ENTRY: Print banner =====
    emit_write_string(&mut code, banner.len(), data_offset);
    
    // ===== MAIN LOOP: Print prompt =====
    let loop_start = code.len();
    
    let prompt_offset = data_offset + banner.len() as i32;
    let pos = code.len();
    emit_write_string_at(&mut code, prompt.len(), prompt_offset, pos);
    
    // ===== READ LOOP =====
    let read_loop = code.len();
    
    // Allocate 1 byte on stack for buffer
    // sub rsp, 8 (align to 8 bytes)
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]);
    
    // Clear the byte
    // mov byte [rsp], 0
    code.extend_from_slice(&[0xc6, 0x04, 0x24, 0x00]);
    
    // Read character: rax=0, rdi=0, rsi=rsp, rdx=1
    // mov rax, SYS_READ
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, SYS_READ, 0x00, 0x00, 0x00]);
    // mov rdi, 0 (stdin)
    code.extend_from_slice(&[0x48, 0xc7, 0xc7, 0x00, 0x00, 0x00, 0x00]);
    // mov rsi, rsp (buffer pointer)
    code.extend_from_slice(&[0x48, 0x89, 0xe6]);
    // mov rdx, 1 (read 1 byte)
    code.extend_from_slice(&[0x48, 0xc7, 0xc2, 0x01, 0x00, 0x00, 0x00]);
    // syscall
    code.extend_from_slice(&[0x0f, 0x05]);
    
    // Check if syscall succeeded (rax == 1 means 1 byte read)
    // test rax, rax
    code.extend_from_slice(&[0x48, 0x85, 0xc0]);
    // jz no_input (if rax == 0, no input available)
    code.extend_from_slice(&[0x74, 0x0e]); // Jump forward 14 bytes to no_input
    
    // We got input! Load the byte
    // mov al, byte [rsp]
    code.extend_from_slice(&[0x8a, 0x04, 0x24]);
    
    // Clean up stack buffer
    // add rsp, 8
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
    
    // Calculate forward jump to skip no_input: we need to jump over the no_input block
    let before_skip_jmp = code.len();
    // Jump to input processing (placeholder, will fix offset)
    code.extend_from_slice(&[0xeb, 0x00]); // Will update offset
    
    // no_input: Clean up stack and yield
    let no_input_start = code.len();
    // add rsp, 8 (clean up buffer)
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
    
    // Yield CPU: mov rax, 158; syscall
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 158, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x0f, 0x05]);
    
    // jmp read_loop (try reading again)
    let jmp_back = (read_loop as i32 - code.len() as i32 - 2) as i8;
    code.extend_from_slice(&[0xeb, jmp_back as u8]);
    
    // Fix the forward jump offset
    let after_no_input = code.len();
    let skip_offset = (after_no_input - before_skip_jmp - 2) as i8;
    code[before_skip_jmp + 1] = skip_offset as u8;
    
    // ===== GOT INPUT:Char is in AL =====
    // mov r12b, al (save character in r12)
    code.extend_from_slice(&[0x41, 0x88, 0xc4]);
    
    // ===== CHECK FOR EXIT =====
    // cmp al, 'e'
    code.extend_from_slice(&[0x3c, b'e']);
    // jne not_exit (skip 12 bytes - the exit block)
    // Exit block is: 7 (mov rax,60) + 3 (xor rdi,rdi) + 2 (syscall) = 12 bytes
    code.extend_from_slice(&[0x75, 0x0c]);
    
    // Exit: mov rax, 60; xor rdi, rdi; syscall
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, SYS_EXIT, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x31, 0xff]);
    code.extend_from_slice(&[0x0f, 0x05]);
    
    // ===== NOT EXIT: Echo character =====
    // sub rsp, 16 (align stack)
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x10]);
    
    // mov [rsp], r12b (char from r12)
    code.extend_from_slice(&[0x44, 0x88, 0x24, 0x24]);
    
    // Write syscall: rax=1, rdi=1, rsi=rsp, rdx=1
    // mov rax, SYS_WRITE
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, SYS_WRITE, 0x00, 0x00, 0x00]);
    // mov rdi, 1
    code.extend_from_slice(&[0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00]);
    // mov rsi, rsp
    code.extend_from_slice(&[0x48, 0x89, 0xe6]);
    // mov rdx, 1
    code.extend_from_slice(&[0x48, 0xc7, 0xc2, 0x01, 0x00, 0x00, 0x00]);
    // syscall
    code.extend_from_slice(&[0x0f, 0x05]);
    
    // add rsp, 16 (restore stack)
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]);
    
    // ===== CHECK FOR NEWLINE =====
    // cmp r12b, '\r'
    code.extend_from_slice(&[0x41, 0x80, 0xfc, 0x0d]);
    // je loop_start
    let jmp_to_loop = (loop_start as i32 - code.len() as i32 - 2) as i8;
    code.extend_from_slice(&[0x74, jmp_to_loop as u8]);
    
    // cmp r12b, '\n'
    code.extend_from_slice(&[0x41, 0x80, 0xfc, 0x0a]);
    // je loop_start
    let jmp_to_loop2 = (loop_start as i32 - code.len() as i32 - 2) as i8;
    code.extend_from_slice(&[0x74, jmp_to_loop2 as u8]);
    
    // ===== CONTINUE READING =====
    // jmp read_loop
    let jmp_to_read = (read_loop as i32 - code.len() as i32 - 2) as i8;
    code.extend_from_slice(&[0xeb, jmp_to_read as u8]);
    
    // ===== PAD TO DATA SECTION =====
    while code.len() < data_offset as usize {
        code.push(0x90);
    }
    
    // ===== DATA =====
    code.extend_from_slice(banner);
    code.extend_from_slice(prompt);
    
    code
}

/// Emit write syscall for string at data_offset
fn emit_write_string(code: &mut alloc::vec::Vec<u8>, len: usize, data_offset: i32) {
    let pos = code.len();
    let rip_offset = data_offset - (pos as i32 + 7);
    
    // lea rsi, [rip + offset]
    code.extend_from_slice(&[0x48, 0x8d, 0x35]);
    code.extend_from_slice(&rip_offset.to_le_bytes()[..4]);
    
    // mov rdx, len
    code.extend_from_slice(&[0x48, 0xc7, 0xc2]);
    code.extend_from_slice(&(len as u32).to_le_bytes());
    
    // mov rdi, 1
    code.extend_from_slice(&[0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00]);
    
    // mov rax, 1
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00]);
    
    // syscall
    code.extend_from_slice(&[0x0f, 0x05]);
}

/// Emit write syscall at specific position
fn emit_write_string_at(code: &mut alloc::vec::Vec<u8>, len: usize, data_offset: i32, pos: usize) {
    let rip_offset = data_offset - (pos as i32 + 7);
    
    code.extend_from_slice(&[0x48, 0x8d, 0x35]);
    code.extend_from_slice(&rip_offset.to_le_bytes()[..4]);
    
    code.extend_from_slice(&[0x48, 0xc7, 0xc2]);
    code.extend_from_slice(&(len as u32).to_le_bytes());
    
    code.extend_from_slice(&[0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x0f, 0x05]);
}

extern crate alloc;
