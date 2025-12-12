//src/reality/intent.rs
//! Intent-driven system calls - programs request what they want, OS decides how

use super::causality::{RealityId, log_event, CausalEventType, CausalEventData, get_total_events};
use crate::memory::fractal::{allocate_fractal_region, SpatialCoord};
use crate::fs::psychicfs::{fs_read, fs_write, fs_list};
use alloc::vec::Vec;

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    // Memory intents
    NeedMemory = 0x0100,
    ReleaseMemory = 0x0101,
    ShareMemory = 0x0102,
    
    // Data intents  
    ReadData = 0x0200,
    WriteData = 0x0201,
    StreamData = 0x0202,
    
    // Task intents
    SpawnTask = 0x0300,
    JoinTask = 0x0301,
    ForkReality = 0x0302,
    MergeReality = 0x0303,
    
    // File intents
    FindFile = 0x0400,
    StoreFile = 0x0401,
    OrganizeFiles = 0x0402,
    
    // Reality/state intents
    Checkpoint = 0x0500,
    Rollback = 0x0501,
    QueryCausality = 0x0502,
}

/// Context for memory requests
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryContext {
    pub size_hint: usize,
    pub access_pattern: AccessPattern,
    pub locality_hint: LocalityHint,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum AccessPattern {
    Sequential = 0,
    Random = 1,
    Temporal = 2,  // Accessed repeatedly soon
    Spatial = 3,   // Accessed near other allocations
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum LocalityHint {
    Hot = 0,       // Frequently accessed
    Warm = 1,      // Occasionally accessed
    Cold = 2,      // Rarely accessed
    Unknown = 3,
}

/// Context for file operations  
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FileContext {
    pub name_hash: u32,
    pub operation_type: u8,
    pub size_hint: u32,
}

#[repr(C)]
pub struct IntentRequest {
    pub intent: Intent,
    pub priority: u8,
    pub context: [u8; 128],
    pub context_len: usize,
}

impl IntentRequest {
    /// Create a memory request with hints
    pub fn memory(size: usize, pattern: AccessPattern, locality: LocalityHint) -> Self {
        let mut context = [0u8; 128];
        let mem_ctx = MemoryContext {
            size_hint: size,
            access_pattern: pattern,
            locality_hint: locality,
        };
        unsafe {
            core::ptr::copy_nonoverlapping(
                &mem_ctx as *const _ as *const u8,
                context.as_mut_ptr(),
                core::mem::size_of::<MemoryContext>()
            );
        }
        Self {
            intent: Intent::NeedMemory,
            priority: 5,
            context,
            context_len: core::mem::size_of::<MemoryContext>(),
        }
    }
    
    /// Parse memory context from request
    fn get_memory_context(&self) -> Option<MemoryContext> {
        if self.context_len >= core::mem::size_of::<MemoryContext>() {
            Some(unsafe {
                core::ptr::read_unaligned(self.context.as_ptr() as *const MemoryContext)
            })
        } else {
            None
        }
    }
}

#[repr(C)]
pub struct IntentResponse {
    pub success: bool,
    pub result_code: i32,
    pub data: [u8; 256],
    pub data_len: usize,
}

impl IntentResponse {
    pub fn success(result_code: i32) -> Self {
        Self {
            success: true,
            result_code,
            data: [0; 256],
            data_len: 0,
        }
    }
    
    pub fn success_with_data(result_code: i32, data: &[u8]) -> Self {
        let mut response = Self::success(result_code);
        let len = data.len().min(256);
        response.data[..len].copy_from_slice(&data[..len]);
        response.data_len = len;
        response
    }
    
    pub fn error(result_code: i32) -> Self {
        Self {
            success: false,
            result_code,
            data: [0; 256],
            data_len: 0,
        }
    }
}

/// Main intent handler - the OS decides HOW to fulfill the intent
pub fn handle_intent(request: &IntentRequest) -> IntentResponse {
    // Log the intent for causal tracking
    log_event(
        CausalEventType::Syscall,
        None,
        CausalEventData { raw: [request.intent as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }
    );
    
    match request.intent {
        Intent::NeedMemory => handle_need_memory(request),
        Intent::ReleaseMemory => handle_release_memory(request),
        Intent::ShareMemory => handle_share_memory(request),
        Intent::ReadData => handle_read_data(request),
        Intent::WriteData => handle_write_data(request),
        Intent::StreamData => handle_stream_data(request),
        Intent::SpawnTask => handle_spawn_task(request),
        Intent::JoinTask => handle_join_task(request),
        Intent::ForkReality => handle_fork_reality(request),
        Intent::MergeReality => handle_merge_reality(request),
        Intent::FindFile => handle_find_file(request),
        Intent::StoreFile => handle_store_file(request),
        Intent::OrganizeFiles => handle_organize_files(request),
        Intent::Checkpoint => handle_checkpoint(request),
        Intent::Rollback => handle_rollback(request),
        Intent::QueryCausality => handle_query_causality(request),
    }
}

/// Handle memory allocation with intent-based optimization
fn handle_need_memory(request: &IntentRequest) -> IntentResponse {
    let ctx = request.get_memory_context();
    
    let size = ctx.map(|c| c.size_hint).unwrap_or(4096);
    
    // Use fractal allocator for spatial allocation
    match allocate_fractal_region(size) {
        Ok(coord) => {
            // Pack spatial coordinates into response
            let mut data = [0u8; 256];
            data[0..8].copy_from_slice(&coord.x.to_le_bytes());
            data[8..16].copy_from_slice(&coord.y.to_le_bytes());
            data[16..24].copy_from_slice(&coord.z.to_le_bytes());
            data[24] = coord.depth;
            
            IntentResponse::success_with_data(0, &data[0..25])
        }
        Err(_) => IntentResponse::error(-12) // ENOMEM
    }
}

fn handle_release_memory(_request: &IntentRequest) -> IntentResponse {
    // Memory is garbage collected during dream state
    // For now, just acknowledge the intent
    IntentResponse::success(0)
}

fn handle_share_memory(_request: &IntentRequest) -> IntentResponse {
    // Shared memory regions use fractal coordinates for lookup
    IntentResponse::error(-38) // ENOSYS - not yet implemented
}

/// Handle data read with psychic prediction
fn handle_read_data(request: &IntentRequest) -> IntentResponse {
    // Extract filename from context
    if request.context_len > 0 {
        let name_len = request.context_len.min(56);
        if let Ok(name) = core::str::from_utf8(&request.context[..name_len]) {
            let trimmed = name.trim_end_matches('\0');
            
            // Record access for prediction
            crate::reality::dream::record_file_access(trimmed.len() as u32);
            
            match fs_read(trimmed) {
                Some(data) => {
                    let len = data.len().min(256);
                    IntentResponse::success_with_data(data.len() as i32, &data[..len])
                }
                None => IntentResponse::error(-2) // ENOENT
            }
        } else {
            IntentResponse::error(-22) // EINVAL
        }
    } else {
        IntentResponse::error(-22) // EINVAL
    }
}

/// Handle data write with intent-based optimization
fn handle_write_data(request: &IntentRequest) -> IntentResponse {
    // Context format: first 56 bytes = filename, rest = data
    if request.context_len > 56 {
        if let Ok(name) = core::str::from_utf8(&request.context[..56]) {
            let trimmed = name.trim_end_matches('\0');
            let data = &request.context[56..request.context_len];
            
            if fs_write(trimmed, data) {
                IntentResponse::success(data.len() as i32)
            } else {
                IntentResponse::error(-5) // EIO
            }
        } else {
            IntentResponse::error(-22)
        }
    } else {
        IntentResponse::error(-22)
    }
}

fn handle_stream_data(_request: &IntentRequest) -> IntentResponse {
    // Streaming data uses ring buffers allocated from fractal memory
    IntentResponse::error(-38) // ENOSYS
}

fn handle_spawn_task(_request: &IntentRequest) -> IntentResponse {
    // Task spawning with capabilities based on intent
    IntentResponse::error(-38) // ENOSYS  
}

fn handle_join_task(_request: &IntentRequest) -> IntentResponse {
    IntentResponse::error(-38) // ENOSYS
}

/// Fork a new reality branch
fn handle_fork_reality(_request: &IntentRequest) -> IntentResponse {
    let new_reality = RealityId::new();
    
    // Log the reality creation
    log_event(
        CausalEventType::Custom,
        None,
        CausalEventData { raw: [0; 32] }
    );
    
    super::causality::set_current_reality(new_reality);
    IntentResponse::success(new_reality.as_u64() as i32)
}

/// Merge back to root reality
fn handle_merge_reality(_request: &IntentRequest) -> IntentResponse {
    super::causality::set_current_reality(RealityId::root());
    IntentResponse::success(0)
}

/// Find file by intent/pattern rather than exact name
fn handle_find_file(request: &IntentRequest) -> IntentResponse {
    // Get partial name or pattern from context
    if request.context_len > 0 {
        let pattern_len = request.context_len.min(56);
        if let Ok(pattern) = core::str::from_utf8(&request.context[..pattern_len]) {
            let pattern = pattern.trim_end_matches('\0');
            
            // List files and find matches
            let files = fs_list();
            for file in &files {
                if file.contains(pattern) {
                    // Return first match
                    let bytes = file.as_bytes();
                    let len = bytes.len().min(256);
                    return IntentResponse::success_with_data(1, &bytes[..len]);
                }
            }
            IntentResponse::error(-2) // ENOENT
        } else {
            IntentResponse::error(-22)
        }
    } else {
        IntentResponse::error(-22)
    }
}

fn handle_store_file(request: &IntentRequest) -> IntentResponse {
    // Similar to write but with intent-based placement
    handle_write_data(request)
}

/// Trigger file reorganization (usually done during dream state)
fn handle_organize_files(_request: &IntentRequest) -> IntentResponse {
    // This triggers dream-state file reorganization
    // Get hot files and move them to faster access regions
    let hot_files = crate::reality::dream::get_hot_files(10);
    IntentResponse::success(hot_files.len() as i32)
}

/// Create a system checkpoint
fn handle_checkpoint(_request: &IntentRequest) -> IntentResponse {
    // Log checkpoint event
    log_event(CausalEventType::Checkpoint, None, CausalEventData { raw: [0; 32] });
    
    // Get current reality and event count
    let reality = RealityId::current();
    let events = get_total_events();
    
    // Return checkpoint ID (combination of reality + event count)
    let checkpoint_id = (reality.as_u64() << 32) | (events as u64 & 0xFFFFFFFF);
    
    IntentResponse::success(checkpoint_id as i32)
}

fn handle_rollback(_request: &IntentRequest) -> IntentResponse {
    // Rollback requires checkpoint restoration (complex, needs full implementation)
    log_event(CausalEventType::Rollback, None, CausalEventData { raw: [0; 32] });
    IntentResponse::error(-38) // ENOSYS for now
}

/// Query the causal chain of events
fn handle_query_causality(request: &IntentRequest) -> IntentResponse {
    let event_count = get_total_events();
    
    // If context contains an event ID, return its causal chain
    if request.context_len >= 8 {
        let event_id = u64::from_le_bytes(request.context[0..8].try_into().unwrap());
        let chain = super::causality::get_causal_chain(event_id);
        
        // Return chain length and first few event IDs
        let mut data = [0u8; 256];
        data[0..8].copy_from_slice(&(chain.len() as u64).to_le_bytes());
        
        for (i, event) in chain.iter().take(30).enumerate() {
            let offset = 8 + (i * 8);
            data[offset..offset+8].copy_from_slice(&event.id.to_le_bytes());
        }
        
        IntentResponse::success_with_data(chain.len() as i32, &data[0..8 + chain.len().min(30) * 8])
    } else {
        IntentResponse::success(event_count as i32)
    }
}