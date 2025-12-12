// ============ src/security/sandbox.rs ============
use crate::process::Pid;
use super::capability::CapabilitySet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SandboxLevel {
    None = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Maximum = 4,
}

pub struct Sandbox {
    pub level: SandboxLevel,
    pub allowed_syscalls: [bool; 256],
    pub allowed_files: alloc::vec::Vec<alloc::string::String>,
    pub network_enabled: bool,
}

impl Sandbox {
    pub fn new(level: SandboxLevel) -> Self {
        let mut allowed_syscalls = [false; 256];
        
        match level {
            SandboxLevel::None => {
                for i in 0..256 {
                    allowed_syscalls[i] = true;
                }
            }
            SandboxLevel::Low => {
                allowed_syscalls[0] = true;  // read
                allowed_syscalls[1] = true;  // write
                allowed_syscalls[60] = true; // exit
            }
            _ => {}
        }
        
        Self {
            level,
            allowed_syscalls,
            allowed_files: alloc::vec::Vec::new(),
            network_enabled: level < SandboxLevel::High,
        }
    }
    
    pub fn check_syscall(&self, syscall_num: u64) -> bool {
        if syscall_num >= 256 {
            return false;
        }
        self.allowed_syscalls[syscall_num as usize]
    }
}