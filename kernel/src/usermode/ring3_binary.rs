//! Embedded Ring 3 Shell Binary
#![allow(dead_code)] // Syscall constants used in code generation
//! Position-independent machine code for a user-mode shell
//!
//! Commands: h=help, l=ls, p=ps, m=mem, u=cpu, k=clear, q=quit

extern crate alloc;

/// Syscall numbers
const SYS_READ: u8 = 0;
const SYS_WRITE: u8 = 1;
const SYS_EXIT: u8 = 60;
const SYS_YIELD: u8 = 158;
const SYS_FS_LIST: u32 = 200;
const SYS_GET_PROCESS_INFO: u32 = 201;
const SYS_GET_MEM_INFO: u32 = 202;
const SYS_GET_CPU_INFO: u32 = 203;
const SYS_CLEAR_SCREEN: u32 = 204;

pub fn get_ring3_shell_binary() -> alloc::vec::Vec<u8> {
    build_ring3_shell()
}

/// Build a simple Ring 3 shell
pub fn build_ring3_shell() -> alloc::vec::Vec<u8> {
    let mut code = alloc::vec::Vec::new();
    
    // Data section offset
    let data_offset: i32 = 512;
    
    // Strings - Fixed command letters to avoid conflicts!
    let banner = b"\n=== Astral OS Ring 3 Shell ===\nCommands: h l p m u k q (type 'h' for help)\n\n";
    let prompt = b"ring3> ";
    let newline = b"\n";
    let help_text = b"\nAvailable commands:\n  h - Help (this message)\n  l - List files\n  p - Process list\n  m - Memory info\n  u - CPU info\n  k - Clear screen\n  q - Quit shell\n\n";
    let unknown = b"Unknown command. Type 'h' for help\n";
    
    // Data offsets
    let banner_off = data_offset;
    let prompt_off = banner_off + banner.len() as i32;
    let newline_off = prompt_off + prompt.len() as i32;
    let help_off = newline_off + newline.len() as i32;
    let unknown_off = help_off + help_text.len() as i32;
    
    // ===== CODE =====
    
    // Print banner
    emit_write_str(&mut code, banner.len(), banner_off);
    
    // ===== MAIN LOOP =====
    let main_loop = code.len();
    
    // Clear r12 (first char storage) - we'll use r12b to store first char
    code.extend_from_slice(&[0x45, 0x31, 0xe4]); // xor r12d, r12d
    // r13 will be 0 initially (not first char yet)
    code.extend_from_slice(&[0x45, 0x31, 0xed]); // xor r13d, r13d
    
    // Print prompt
    emit_write_str(&mut code, prompt.len(), prompt_off);
    
    // ===== READ LOOP =====
    let read_loop = code.len();
    
    // sub rsp, 16 (buffer for one char)
    code.extend_from_slice(&[0x48, 0x83, 0xec, 0x10]);
    
    // Clear buffer byte
    code.extend_from_slice(&[0xc6, 0x04, 0x24, 0x00]); // mov byte [rsp], 0
    
    // SYS_READ: rax=0, rdi=0, rsi=rsp, rdx=1
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, SYS_READ, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0xc7, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x89, 0xe6]); // mov rsi, rsp
    code.extend_from_slice(&[0x48, 0xc7, 0xc2, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x0f, 0x05]);
    
    // Check if got input
    code.extend_from_slice(&[0x48, 0x85, 0xc0]); // test rax, rax
    let jz_yield = code.len();
    code.extend_from_slice(&[0x0F, 0x84, 0x00, 0x00, 0x00, 0x00]); // JZ placeholder
    
    // Load the character into al
    code.extend_from_slice(&[0x8a, 0x04, 0x24]); // mov al, [rsp]
    
    // Check for Enter
    code.extend_from_slice(&[0x3c, 0x0d]); // cmp al, '\r'
    let je_process = code.len();
    code.extend_from_slice(&[0x0F, 0x84, 0x00, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x3c, 0x0a]); // cmp al, '\n'
    let je_process2 = code.len();
    code.extend_from_slice(&[0x0F, 0x84, 0x00, 0x00, 0x00, 0x00]);
    
    // If this is the first character (r13 == 0), save it to r12b
    code.extend_from_slice(&[0x4d, 0x85, 0xed]); // test r13, r13
    let jnz_not_first = code.len();
    code.extend_from_slice(&[0x0F, 0x85, 0x00, 0x00, 0x00, 0x00]); // JNZ skip
    
    // Save first char: mov r12b, al
    code.extend_from_slice(&[0x41, 0x88, 0xc4]); // mov r12b, al
    // Set r13 = 1 to indicate we have first char
    code.extend_from_slice(&[0x49, 0xc7, 0xc5, 0x01, 0x00, 0x00, 0x00]); // mov r13, 1
    
    let not_first = code.len();
    
    // Echo the character
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, SYS_WRITE, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x89, 0xe6]); // mov rsi, rsp
    code.extend_from_slice(&[0x48, 0xc7, 0xc2, 0x01, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x0f, 0x05]);
    
    // Cleanup
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]); // add rsp, 16
    
    // Jump back to read_loop
    emit_near_jmp(&mut code, read_loop);
    
    // ===== YIELD =====
    let yield_pos = code.len();
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]); // add rsp, 16
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, SYS_YIELD, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x0f, 0x05]);
    emit_near_jmp(&mut code, read_loop);
    
    // ===== PROCESS COMMAND =====
    let process_cmd = code.len();
    code.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]); // add rsp, 16
    
    // Print newline
    emit_write_str(&mut code, 1, newline_off);
    
    // Check if we got any input (r13 != 0)
    code.extend_from_slice(&[0x4d, 0x85, 0xed]); // test r13, r13
    let jz_main = code.len();
    code.extend_from_slice(&[0x0F, 0x84, 0x00, 0x00, 0x00, 0x00]); // JZ main_loop
    
    // Check first character (r12b)
    
    // 'h' = help
    code.extend_from_slice(&[0x41, 0x80, 0xfc, b'h']);
    let jne_check_l = code.len();
    code.extend_from_slice(&[0x0F, 0x85, 0x00, 0x00, 0x00, 0x00]);
    emit_write_str(&mut code, help_text.len(), help_off);
    emit_near_jmp(&mut code, main_loop);
    
    // 'l' = ls
    let check_l = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xfc, b'l']);
    let jne_check_p = code.len();
    code.extend_from_slice(&[0x0F, 0x85, 0x00, 0x00, 0x00, 0x00]);
    emit_syscall(&mut code, SYS_FS_LIST);
    emit_near_jmp(&mut code, main_loop);
    
    // 'p' = ps
    let check_p = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xfc, b'p']);
    let jne_check_m = code.len();
    code.extend_from_slice(&[0x0F, 0x85, 0x00, 0x00, 0x00, 0x00]);
    emit_syscall(&mut code, SYS_GET_PROCESS_INFO);
    emit_near_jmp(&mut code, main_loop);
    
    // 'm' = mem
    let check_m = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xfc, b'm']);
    let jne_check_u = code.len();
    code.extend_from_slice(&[0x0F, 0x85, 0x00, 0x00, 0x00, 0x00]);
    emit_syscall(&mut code, SYS_GET_MEM_INFO);
    emit_near_jmp(&mut code, main_loop);
    
    // 'u' = cpu (changed from 'c' to avoid conflict)
    let check_u = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xfc, b'u']);
    let jne_check_k = code.len();
    code.extend_from_slice(&[0x0F, 0x85, 0x00, 0x00, 0x00, 0x00]);
    emit_syscall(&mut code, SYS_GET_CPU_INFO);
    emit_near_jmp(&mut code, main_loop);
    
    // 'k' = clear (changed from 'x' for consistency)
    let check_k = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xfc, b'k']);
    let jne_check_q = code.len();
    code.extend_from_slice(&[0x0F, 0x85, 0x00, 0x00, 0x00, 0x00]);
    emit_syscall(&mut code, SYS_CLEAR_SCREEN);
    emit_near_jmp(&mut code, main_loop);
    
    // 'q' = quit (changed from 'e' for clarity)
    let check_q = code.len();
    code.extend_from_slice(&[0x41, 0x80, 0xfc, b'q']);
    let jne_unknown = code.len();
    code.extend_from_slice(&[0x0F, 0x85, 0x00, 0x00, 0x00, 0x00]);
    // Exit: syscall(60, 0)
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, SYS_EXIT, 0x00, 0x00, 0x00]);
    code.extend_from_slice(&[0x48, 0x31, 0xff]); // xor rdi, rdi
    code.extend_from_slice(&[0x0f, 0x05]);
    
    // Unknown command
    let unknown_pos = code.len();
    emit_write_str(&mut code, unknown.len(), unknown_off);
    emit_near_jmp(&mut code, main_loop);
    
    // Fix jumps
    fix_jmp(&mut code, jz_yield, yield_pos);
    fix_jmp(&mut code, je_process, process_cmd);
    fix_jmp(&mut code, je_process2, process_cmd);
    fix_jmp(&mut code, jnz_not_first, not_first);
    fix_jmp(&mut code, jz_main, main_loop);
    fix_jmp(&mut code, jne_check_l, check_l);
    fix_jmp(&mut code, jne_check_p, check_p);
    fix_jmp(&mut code, jne_check_m, check_m);
    fix_jmp(&mut code, jne_check_u, check_u);
    fix_jmp(&mut code, jne_check_k, check_k);
    fix_jmp(&mut code, jne_check_q, check_q);
    fix_jmp(&mut code, jne_unknown, unknown_pos);
    
    // Pad to data
    while code.len() < data_offset as usize {
        code.push(0x90);
    }
    
    // Data section
    code.extend_from_slice(banner);
    code.extend_from_slice(prompt);
    code.extend_from_slice(newline);
    code.extend_from_slice(help_text);
    code.extend_from_slice(unknown);
    
    code
}

fn emit_write_str(code: &mut alloc::vec::Vec<u8>, len: usize, offset: i32) {
    let pos = code.len();
    let rip_off = offset - (pos as i32 + 7);
    code.extend_from_slice(&[0x48, 0x8d, 0x35]); // lea rsi, [rip+off]
    code.extend_from_slice(&rip_off.to_le_bytes()[..4]);
    code.extend_from_slice(&[0x48, 0xc7, 0xc2]); // mov rdx, len
    code.extend_from_slice(&(len as u32).to_le_bytes());
    code.extend_from_slice(&[0x48, 0xc7, 0xc7, 0x01, 0x00, 0x00, 0x00]); // mov rdi, 1
    code.extend_from_slice(&[0x48, 0xc7, 0xc0, 0x01, 0x00, 0x00, 0x00]); // mov rax, 1
    code.extend_from_slice(&[0x0f, 0x05]); // syscall
}

fn emit_syscall(code: &mut alloc::vec::Vec<u8>, num: u32) {
    code.extend_from_slice(&[0x48, 0xc7, 0xc0]); // mov rax, num
    code.extend_from_slice(&num.to_le_bytes());
    code.extend_from_slice(&[0x0f, 0x05]); // syscall
}

fn emit_near_jmp(code: &mut alloc::vec::Vec<u8>, target: usize) {
    let cur = code.len();
    let rel = (target as i32) - (cur as i32) - 5;
    code.push(0xE9);
    code.extend_from_slice(&rel.to_le_bytes());
}

fn fix_jmp(code: &mut alloc::vec::Vec<u8>, jmp_off: usize, target: usize) {
    let op = code[jmp_off];
    let rel_off = if op == 0xE9 { jmp_off + 1 } else { jmp_off + 2 };
    let instr_end = rel_off + 4;
    let rel = (target as i32) - (instr_end as i32);
    let b = rel.to_le_bytes();
    code[rel_off] = b[0];
    code[rel_off + 1] = b[1];
    code[rel_off + 2] = b[2];
    code[rel_off + 3] = b[3];
}
