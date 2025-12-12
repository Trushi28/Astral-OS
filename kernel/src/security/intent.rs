// src/security/intent.rs

use super::capability::Capability;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Debug)]
pub struct Intent {
    pub action: IntentAction,
    pub target: Option<String>,
    pub parameters: Vec<(String, String)>,
    pub priority: IntentPriority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntentAction {
    ReadFile,
    WriteFile,
    ExecuteFile,
    CreateFile,
    DeleteFile,
    ConnectNetwork,
    BindNetwork,
    SpawnProcess,
    AllocateMemory,
    CreateGraphics,
    Shutdown,
    Reboot,
    ForkReality,
    MergeReality,
    QueryCausality,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum IntentPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Intent {
    pub fn new(action: IntentAction) -> Self {
        Self {
            action,
            target: None,
            parameters: Vec::new(),
            priority: IntentPriority::Normal,
        }
    }
    
    pub fn with_target(mut self, target: String) -> Self {
        self.target = Some(target);
        self
    }
    
    pub fn with_priority(mut self, priority: IntentPriority) -> Self {
        self.priority = priority;
        self
    }
    
    pub fn add_parameter(mut self, key: String, value: String) -> Self {
        self.parameters.push((key, value));
        self
    }
    
    /// Check if intent matches a capability
    pub fn matches_capability(&self, cap: Capability) -> bool {
        match (self.action, cap) {
            (IntentAction::ReadFile, Capability::FileRead) => true,
            (IntentAction::WriteFile, Capability::FileWrite) => true,
            (IntentAction::ExecuteFile, Capability::FileExecute) => true,
            (IntentAction::CreateFile, Capability::FileCreate) => true,
            (IntentAction::DeleteFile, Capability::FileDelete) => true,
            (IntentAction::ConnectNetwork, Capability::NetConnect) => true,
            (IntentAction::BindNetwork, Capability::NetBind) => true,
            (IntentAction::SpawnProcess, Capability::ProcessSpawn) => true,
            (IntentAction::AllocateMemory, Capability::MemoryAllocate) => true,
            (IntentAction::CreateGraphics, Capability::GraphicsCreate) => true,
            (IntentAction::Shutdown, Capability::SystemShutdown) => true,
            (IntentAction::Reboot, Capability::SystemReboot) => true,
            (IntentAction::ForkReality, Capability::RealityFork) => true,
            (IntentAction::MergeReality, Capability::RealityMerge) => true,
            (IntentAction::QueryCausality, Capability::CausalityQuery) => true,
            _ => false,
        }
    }
    
    /// Get required capability for this intent
    pub fn required_capability(&self) -> Option<Capability> {
        match self.action {
            IntentAction::ReadFile => Some(Capability::FileRead),
            IntentAction::WriteFile => Some(Capability::FileWrite),
            IntentAction::ExecuteFile => Some(Capability::FileExecute),
            IntentAction::CreateFile => Some(Capability::FileCreate),
            IntentAction::DeleteFile => Some(Capability::FileDelete),
            IntentAction::ConnectNetwork => Some(Capability::NetConnect),
            IntentAction::BindNetwork => Some(Capability::NetBind),
            IntentAction::SpawnProcess => Some(Capability::ProcessSpawn),
            IntentAction::AllocateMemory => Some(Capability::MemoryAllocate),
            IntentAction::CreateGraphics => Some(Capability::GraphicsCreate),
            IntentAction::Shutdown => Some(Capability::SystemShutdown),
            IntentAction::Reboot => Some(Capability::SystemReboot),
            IntentAction::ForkReality => Some(Capability::RealityFork),
            IntentAction::MergeReality => Some(Capability::RealityMerge),
            IntentAction::QueryCausality => Some(Capability::CausalityQuery),
            IntentAction::Custom => None,
        }
    }
}

/// Parse intent from string (DSL-like syntax)
pub fn parse_intent(intent_str: &str) -> Option<Intent> {
    // Simple parsing: "action:target?param1=value1&param2=value2"
    let parts: Vec<&str> = intent_str.splitn(2, ':').collect();
    
    if parts.is_empty() {
        return None;
    }
    
    let action = match parts[0] {
        "read" => IntentAction::ReadFile,
        "write" => IntentAction::WriteFile,
        "exec" => IntentAction::ExecuteFile,
        "create" => IntentAction::CreateFile,
        "delete" => IntentAction::DeleteFile,
        "connect" => IntentAction::ConnectNetwork,
        "bind" => IntentAction::BindNetwork,
        "spawn" => IntentAction::SpawnProcess,
        "alloc" => IntentAction::AllocateMemory,
        "graphics" => IntentAction::CreateGraphics,
        "shutdown" => IntentAction::Shutdown,
        "reboot" => IntentAction::Reboot,
        "reality.fork" => IntentAction::ForkReality,
        "reality.merge" => IntentAction::MergeReality,
        "causality.query" => IntentAction::QueryCausality,
        _ => IntentAction::Custom,
    };
    
    let mut intent = Intent::new(action);
    
    if parts.len() > 1 {
        let target_and_params: Vec<&str> = parts[1].splitn(2, '?').collect();
        
        if !target_and_params[0].is_empty() {
            intent.target = Some(String::from(target_and_params[0]));
        }
        
        if target_and_params.len() > 1 {
            for param in target_and_params[1].split('&') {
                let kv: Vec<&str> = param.splitn(2, '=').collect();
                if kv.len() == 2 {
                    intent.parameters.push((
                        String::from(kv[0]),
                        String::from(kv[1])
                    ));
                }
            }
        }
    }
    
    Some(intent)
}