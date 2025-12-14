// src/security/capability.rs


#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum Capability {
    // File system
    FileRead = 1 << 0,
    FileWrite = 1 << 1,
    FileExecute = 1 << 2,
    FileCreate = 1 << 3,
    FileDelete = 1 << 4,
    
    // Network
    NetConnect = 1 << 10,
    NetBind = 1 << 11,
    NetRawSocket = 1 << 12,
    
    // Process
    ProcessSpawn = 1 << 20,
    ProcessKill = 1 << 21,
    ProcessDebug = 1 << 22,
    
    // Memory
    MemoryAllocate = 1 << 30,
    MemoryMap = 1 << 31,
    MemoryExecute = 1 << 32,
    
    // Graphics
    GraphicsCreate = 1 << 40,
    GraphicsBlit = 1 << 41,
    GraphicsPresent = 1 << 42,
    
    // System
    SystemShutdown = 1 << 50,
    SystemReboot = 1 << 51,
    SystemTime = 1 << 52,
    
    // Reality Engine
    RealityFork = 1 << 60,
    RealityMerge = 1 << 61,
    CausalityQuery = 1 << 62,
}

impl Capability {
    /// Check if capability is sensitive (requires high trust)
    pub fn is_sensitive(&self) -> bool {
        matches!(
            self,
            Capability::FileExecute
                | Capability::NetRawSocket
                | Capability::ProcessDebug
                | Capability::MemoryExecute
                | Capability::SystemShutdown
                | Capability::SystemReboot
        )
    }
    
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Capability::FileRead => "File Read",
            Capability::FileWrite => "File Write",
            Capability::FileExecute => "File Execute",
            Capability::FileCreate => "File Create",
            Capability::FileDelete => "File Delete",
            Capability::NetConnect => "Network Connect",
            Capability::NetBind => "Network Bind",
            Capability::NetRawSocket => "Raw Socket",
            Capability::ProcessSpawn => "Process Spawn",
            Capability::ProcessKill => "Process Kill",
            Capability::ProcessDebug => "Process Debug",
            Capability::MemoryAllocate => "Memory Allocate",
            Capability::MemoryMap => "Memory Map",
            Capability::MemoryExecute => "Memory Execute",
            Capability::GraphicsCreate => "Graphics Create",
            Capability::GraphicsBlit => "Graphics Blit",
            Capability::GraphicsPresent => "Graphics Present",
            Capability::SystemShutdown => "System Shutdown",
            Capability::SystemReboot => "System Reboot",
            Capability::SystemTime => "System Time",
            Capability::RealityFork => "Reality Fork",
            Capability::RealityMerge => "Reality Merge",
            Capability::CausalityQuery => "Causality Query",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CapabilitySet {
    bits: u64,
}

impl CapabilitySet {
    pub const fn new() -> Self {
        Self { bits: 0 }
    }
    
    pub const fn all() -> Self {
        Self { bits: !0 }
    }
    
    pub fn default() -> Self {
        // Default capabilities for normal processes
        let mut set = Self::new();
        set.bits |= Capability::FileRead as u64;
        set.bits |= Capability::MemoryAllocate as u64;
        set.bits |= Capability::GraphicsCreate as u64;
        set.bits |= Capability::GraphicsBlit as u64;
        set.bits |= Capability::GraphicsPresent as u64;
        set
    }
    
    pub fn has(&self, cap: Capability) -> bool {
        (self.bits & (cap as u64)) != 0
    }
    
    pub fn add(&mut self, cap: Capability) {
        self.bits |= cap as u64;
    }
    
    pub fn remove(&mut self, cap: Capability) {
        self.bits &= !(cap as u64);
    }
    
    pub fn clear(&mut self) {
        self.bits = 0;
    }
    
    pub fn union(&self, other: &CapabilitySet) -> CapabilitySet {
        CapabilitySet {
            bits: self.bits | other.bits,
        }
    }
    
    pub fn intersection(&self, other: &CapabilitySet) -> CapabilitySet {
        CapabilitySet {
            bits: self.bits & other.bits,
        }
    }
}