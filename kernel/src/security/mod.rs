// src/security/mod.rs
//! Intent-based security model with capability evolution

pub mod capability;
pub mod intent;
pub mod sandbox;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use spin::Mutex;
use crate::process::Pid;
use capability::{Capability, CapabilitySet};
use intent::Intent;

/// Security context for a process
#[derive(Clone)]
pub struct SecurityContext {
    pub pid: Pid,
    pub capabilities: CapabilitySet,
    pub intent_history: Vec<Intent>,
    pub trust_score: u32,
    pub sandbox_level: SandboxLevel,
    
    // Adaptive features
    pub behavior_patterns: BehaviorPatterns,
    pub capability_requests: Vec<CapabilityRequest>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SandboxLevel {
    None = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Maximum = 4,
}

#[derive(Clone)]
pub struct BehaviorPatterns {
    pub file_access_count: u32,
    pub network_access_count: u32,
    pub process_spawn_count: u32,
    pub memory_allocations: u32,
    pub syscall_distribution: BTreeMap<u64, u32>,
    pub anomaly_score: u32,
}

impl BehaviorPatterns {
    pub fn new() -> Self {
        Self {
            file_access_count: 0,
            network_access_count: 0,
            process_spawn_count: 0,
            memory_allocations: 0,
            syscall_distribution: BTreeMap::new(),
            anomaly_score: 0,
        }
    }
    
    pub fn record_syscall(&mut self, syscall_num: u64) {
        *self.syscall_distribution.entry(syscall_num).or_insert(0) += 1;
    }
    
    pub fn update_anomaly_score(&mut self) {
        // Simple heuristic: high syscall rate = potential anomaly
        let total_syscalls: u32 = self.syscall_distribution.values().sum();
        
        if total_syscalls > 10000 {
            self.anomaly_score += 1;
        }
        
        // Cap at 100
        if self.anomaly_score > 100 {
            self.anomaly_score = 100;
        }
    }
}

#[derive(Clone, Debug)]
pub struct CapabilityRequest {
    pub capability: Capability,
    pub intent: Intent,
    pub timestamp: u64,
    pub granted: bool,
    pub reason: Option<&'static str>,
}

impl SecurityContext {
    pub fn new(pid: Pid) -> Self {
        Self {
            pid,
            capabilities: CapabilitySet::default(),
            intent_history: Vec::new(),
            trust_score: 50, // Start neutral
            sandbox_level: SandboxLevel::Medium,
            behavior_patterns: BehaviorPatterns::new(),
            capability_requests: Vec::new(),
        }
    }
    
    pub fn new_trusted(pid: Pid) -> Self {
        let mut ctx = Self::new(pid);
        ctx.trust_score = 100;
        ctx.sandbox_level = SandboxLevel::None;
        ctx.capabilities = CapabilitySet::all();
        ctx
    }
    
    pub fn check_capability(&self, cap: Capability) -> bool {
        self.capabilities.has(cap)
    }
    
    pub fn grant_capability(&mut self, cap: Capability, reason: &'static str) {
        self.capabilities.add(cap);
        
        // Record the grant
        if let Some(last_req) = self.capability_requests.last_mut() {
            if last_req.capability == cap && !last_req.granted {
                last_req.granted = true;
                last_req.reason = Some(reason);
            }
        }
    }
    
    pub fn revoke_capability(&mut self, cap: Capability) {
        self.capabilities.remove(cap);
    }
    
    pub fn request_capability(&mut self, cap: Capability, intent: Intent) -> bool {
        // Record request
        self.capability_requests.push(CapabilityRequest {
            capability: cap,
            intent: intent.clone(),
            timestamp: crate::get_timestamp(),
            granted: false,
            reason: None,
        });
        
        // Evaluate request based on:
        // 1. Current trust score
        // 2. Intent validity
        // 3. Behavior patterns
        // 4. Capability sensitivity
        
        let should_grant = self.evaluate_capability_request(cap, &intent);
        
        if should_grant {
            self.grant_capability(cap, "Intent-based grant");
        }
        
        should_grant
    }
    
    fn evaluate_capability_request(&self, cap: Capability, intent: &Intent) -> bool {
        // High trust score = more likely to grant
        if self.trust_score >= 80 {
            return true;
        }
        
        // Check if intent matches capability
        if !intent.matches_capability(cap) {
            return false;
        }
        
        // Check anomaly score
        if self.behavior_patterns.anomaly_score > 70 {
            return false; // Too suspicious
        }
        
        // Sensitive capabilities require higher trust
        if cap.is_sensitive() && self.trust_score < 60 {
            return false;
        }
        
        // Default: grant based on trust score
        self.trust_score >= 50
    }
    
    pub fn record_intent(&mut self, intent: Intent) {
        self.intent_history.push(intent);
        
        // Keep only recent history
        if self.intent_history.len() > 1000 {
            self.intent_history.remove(0);
        }
    }
    
    pub fn adjust_trust_score(&mut self, delta: i32) {
        let new_score = (self.trust_score as i32 + delta).clamp(0, 100);
        self.trust_score = new_score as u32;
        
        // Adjust sandbox level based on trust
        self.sandbox_level = match self.trust_score {
            0..=20 => SandboxLevel::Maximum,
            21..=40 => SandboxLevel::High,
            41..=60 => SandboxLevel::Medium,
            61..=80 => SandboxLevel::Low,
            81..=100 => SandboxLevel::None,
            _ => SandboxLevel::Medium,
        };
    }
}

// Global security manager
static SECURITY_MANAGER: Mutex<SecurityManager> = Mutex::new(SecurityManager::new());

pub struct SecurityManager {
    contexts: BTreeMap<u64, SecurityContext>,
}

impl SecurityManager {
    pub const fn new() -> Self {
        Self {
            contexts: BTreeMap::new(),
        }
    }
    
    pub fn create_context(&mut self, pid: Pid) -> &mut SecurityContext {
        let ctx = SecurityContext::new(pid);
        self.contexts.insert(pid.as_u64(), ctx);
        self.contexts.get_mut(&pid.as_u64()).unwrap()
    }
    
    pub fn create_trusted_context(&mut self, pid: Pid) -> &mut SecurityContext {
        let ctx = SecurityContext::new_trusted(pid);
        self.contexts.insert(pid.as_u64(), ctx);
        self.contexts.get_mut(&pid.as_u64()).unwrap()
    }
    
    pub fn get_context(&self, pid: Pid) -> Option<&SecurityContext> {
        self.contexts.get(&pid.as_u64())
    }
    
    pub fn get_context_mut(&mut self, pid: Pid) -> Option<&mut SecurityContext> {
        self.contexts.get_mut(&pid.as_u64())
    }
    
    pub fn remove_context(&mut self, pid: Pid) {
        self.contexts.remove(&pid.as_u64());
    }
}

// Public API
pub fn create_security_context(pid: Pid) {
    let mut mgr = SECURITY_MANAGER.lock();
    mgr.create_context(pid);
}

pub fn create_trusted_context(pid: Pid) {
    let mut mgr = SECURITY_MANAGER.lock();
    mgr.create_trusted_context(pid);
}

pub fn check_capability(pid: Pid, cap: Capability) -> bool {
    let mgr = SECURITY_MANAGER.lock();
    if let Some(ctx) = mgr.get_context(pid) {
        ctx.check_capability(cap)
    } else {
        false
    }
}

pub fn request_capability(pid: Pid, cap: Capability, intent: Intent) -> bool {
    let mut mgr = SECURITY_MANAGER.lock();
    if let Some(ctx) = mgr.get_context_mut(pid) {
        ctx.request_capability(cap, intent)
    } else {
        false
    }
}

pub fn record_syscall(pid: Pid, syscall_num: u64) {
    let mut mgr = SECURITY_MANAGER.lock();
    if let Some(ctx) = mgr.get_context_mut(pid) {
        ctx.behavior_patterns.record_syscall(syscall_num);
    }
}

pub fn get_trust_score(pid: Pid) -> u32 {
    let mgr = SECURITY_MANAGER.lock();
    if let Some(ctx) = mgr.get_context(pid) {
        ctx.trust_score
    } else {
        0
    }
}

pub fn adjust_trust_score(pid: Pid, delta: i32) {
    let mut mgr = SECURITY_MANAGER.lock();
    if let Some(ctx) = mgr.get_context_mut(pid) {
        ctx.adjust_trust_score(delta);
    }
}