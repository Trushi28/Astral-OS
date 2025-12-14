//! IPC - Inter-Process Communication
//! 
//! Blocking channels with scheduler integration for usermode apps.

use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use spin::Mutex;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use crate::process::{Pid, ProcessState, process_table};

/// Maximum messages in channel buffer
const CHANNEL_CAPACITY: usize = 64;
/// Maximum message data size
const MESSAGE_SIZE: usize = 256;

static NEXT_CHANNEL_ID: AtomicU64 = AtomicU64::new(1);

/// Wait queue for blocking operations
pub struct WaitQueue {
    waiters: Mutex<Vec<Pid>>,
}

impl WaitQueue {
    pub const fn new() -> Self {
        Self {
            waiters: Mutex::new(Vec::new()),
        }
    }
    
    /// Add process to wait queue and block it
    pub fn wait(&self, pid: Pid) {
        self.waiters.lock().push(pid);
        
        // Block the process
        let mut table = process_table().lock();
        if let Some(proc) = table.get_mut(pid) {
            proc.state = ProcessState::Blocked;
            proc.wait_reason = WaitReason::IpcRecv as u64;
        }
        drop(table);
        
        // Yield to scheduler
        crate::process::scheduler::yield_cpu();
    }
    
    /// Wake one waiter (FIFO order)
    pub fn wake_one(&self) -> Option<Pid> {
        let pid = {
            let mut waiters = self.waiters.lock();
            if waiters.is_empty() {
                return None;
            }
            waiters.remove(0)
        };
        
        // Unblock the process
        let mut table = process_table().lock();
        if let Some(proc) = table.get_mut(pid) {
            proc.state = ProcessState::Ready;
            proc.wait_reason = 0;
        }
        
        Some(pid)
    }
    
    /// Wake all waiters
    pub fn wake_all(&self) {
        let pids: Vec<Pid> = self.waiters.lock().drain(..).collect();
        
        let mut table = process_table().lock();
        for pid in pids {
            if let Some(proc) = table.get_mut(pid) {
                proc.state = ProcessState::Ready;
                proc.wait_reason = 0;
            }
        }
    }
    
    pub fn is_empty(&self) -> bool {
        self.waiters.lock().is_empty()
    }
}

/// Wait reasons for blocked processes
#[repr(u64)]
pub enum WaitReason {
    None = 0,
    IpcSend = 1,   // Blocked on full channel
    IpcRecv = 2,   // Blocked on empty channel
    Mutex = 3,     // Blocked on mutex
    Sleep = 4,     // Timed sleep
    Input = 5,     // Waiting for input
}

/// IPC Message
#[derive(Clone)]
pub struct Message {
    pub msg_type: u32,
    pub sender: Pid,
    pub data: [u8; MESSAGE_SIZE],
    pub len: usize,
}

impl Message {
    pub fn new(msg_type: u32, sender: Pid, data: &[u8]) -> Self {
        let mut msg = Self {
            msg_type,
            sender,
            data: [0; MESSAGE_SIZE],
            len: data.len().min(MESSAGE_SIZE),
        };
        msg.data[..msg.len].copy_from_slice(&data[..msg.len]);
        msg
    }
    
    pub fn data(&self) -> &[u8] {
        &self.data[..self.len]
    }
}

/// Ring buffer for messages
pub struct MessageBuffer {
    messages: [Option<Message>; CHANNEL_CAPACITY],
    head: usize,
    tail: usize,
    count: usize,
}

impl MessageBuffer {
    pub const fn new() -> Self {
        const NONE: Option<Message> = None;
        Self {
            messages: [NONE; CHANNEL_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
        }
    }
    
    pub fn push(&mut self, msg: Message) -> bool {
        if self.count >= CHANNEL_CAPACITY {
            return false;
        }
        self.messages[self.tail] = Some(msg);
        self.tail = (self.tail + 1) % CHANNEL_CAPACITY;
        self.count += 1;
        true
    }
    
    pub fn pop(&mut self) -> Option<Message> {
        if self.count == 0 {
            return None;
        }
        let msg = self.messages[self.head].take();
        self.head = (self.head + 1) % CHANNEL_CAPACITY;
        self.count -= 1;
        msg
    }
    
    pub fn is_full(&self) -> bool {
        self.count >= CHANNEL_CAPACITY
    }
    
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// IPC Channel with blocking semantics
pub struct Channel {
    pub id: u64,
    buffer: Mutex<MessageBuffer>,
    send_waiters: WaitQueue,  // Blocked senders (buffer full)
    recv_waiters: WaitQueue,  // Blocked receivers (buffer empty)
    closed: AtomicBool,
}

impl Channel {
    pub fn new() -> Self {
        Self {
            id: NEXT_CHANNEL_ID.fetch_add(1, Ordering::Relaxed),
            buffer: Mutex::new(MessageBuffer::new()),
            send_waiters: WaitQueue::new(),
            recv_waiters: WaitQueue::new(),
            closed: AtomicBool::new(false),
        }
    }
    
    /// Send message (blocking if full)
    pub fn send(&self, msg: Message) -> Result<(), IpcError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(IpcError::ChannelClosed);
        }
        
        loop {
            {
                let mut buf = self.buffer.lock();
                if buf.push(msg.clone()) {
                    drop(buf);
                    // Wake one waiting receiver
                    self.recv_waiters.wake_one();
                    return Ok(());
                }
            }
            
            // Buffer full - block sender
            if let Some(pid) = crate::process::get_current_pid() {
                self.send_waiters.wait(pid);
            } else {
                return Err(IpcError::WouldBlock);
            }
            
            // Check if closed while waiting
            if self.closed.load(Ordering::Relaxed) {
                return Err(IpcError::ChannelClosed);
            }
        }
    }
    
    /// Receive message (blocking if empty)
    pub fn recv(&self) -> Result<Message, IpcError> {
        loop {
            {
                let mut buf = self.buffer.lock();
                if let Some(msg) = buf.pop() {
                    drop(buf);
                    // Wake one waiting sender
                    self.send_waiters.wake_one();
                    return Ok(msg);
                }
            }
            
            // Check if closed
            if self.closed.load(Ordering::Relaxed) {
                return Err(IpcError::ChannelClosed);
            }
            
            // Buffer empty - block receiver
            if let Some(pid) = crate::process::get_current_pid() {
                self.recv_waiters.wait(pid);
            } else {
                return Err(IpcError::WouldBlock);
            }
        }
    }
    
    /// Try receive without blocking
    pub fn try_recv(&self) -> Result<Message, IpcError> {
        let mut buf = self.buffer.lock();
        if let Some(msg) = buf.pop() {
            self.send_waiters.wake_one();
            Ok(msg)
        } else if self.closed.load(Ordering::Relaxed) {
            Err(IpcError::ChannelClosed)
        } else {
            Err(IpcError::WouldBlock)
        }
    }
    
    /// Close channel
    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        self.send_waiters.wake_all();
        self.recv_waiters.wake_all();
    }
}

#[derive(Debug, Clone, Copy)]
pub enum IpcError {
    ChannelClosed,
    WouldBlock,
    InvalidChannel,
    PermissionDenied,
}

// Global channel registry
static CHANNELS: Mutex<BTreeMap<u64, Channel>> = Mutex::new(BTreeMap::new());

/// Create a new IPC channel
pub fn create_channel() -> u64 {
    let channel = Channel::new();
    let id = channel.id;
    CHANNELS.lock().insert(id, channel);
    id
}

/// Send to channel by ID
pub fn channel_send(channel_id: u64, msg: Message) -> Result<(), IpcError> {
    let channels = CHANNELS.lock();
    if let Some(ch) = channels.get(&channel_id) {
        // Need to drop lock before blocking
        let ch_ptr = ch as *const Channel;
        drop(channels);
        unsafe { (*ch_ptr).send(msg) }
    } else {
        Err(IpcError::InvalidChannel)
    }
}

/// Receive from channel by ID
pub fn channel_recv(channel_id: u64) -> Result<Message, IpcError> {
    let channels = CHANNELS.lock();
    if let Some(ch) = channels.get(&channel_id) {
        let ch_ptr = ch as *const Channel;
        drop(channels);
        unsafe { (*ch_ptr).recv() }
    } else {
        Err(IpcError::InvalidChannel)
    }
}

/// Close and remove channel
pub fn close_channel(channel_id: u64) {
    if let Some(ch) = CHANNELS.lock().remove(&channel_id) {
        ch.close();
    }
}
