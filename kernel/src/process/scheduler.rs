//src/process/scheduler.rs
use super::{Pid, ProcessState, process_table, get_current_pid, set_current_pid};
use alloc::collections::VecDeque;
use spin::Mutex;

pub struct Scheduler {
    ready_queue: VecDeque<Pid>,
    current_time_slice: u64,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
            current_time_slice: 0,
        }
    }
    
    pub fn add_process(&mut self, pid: Pid) {
        if !self.ready_queue.contains(&pid) {
            self.ready_queue.push_back(pid);
        }
    }
    
    pub fn remove_process(&mut self, pid: Pid) {
        self.ready_queue.retain(|&p| p != pid);
    }
    
    pub fn schedule(&mut self) -> Option<Pid> {
        // Decrement time slice
        if self.current_time_slice > 0 {
            self.current_time_slice -= 1;
            
            // If still has time, keep current process running
            if self.current_time_slice > 0 {
                if let Some(current) = get_current_pid() {
                    let table = process_table().lock();
                    if let Some(proc) = table.processes.iter().flatten().find(|p| p.pid == current) {
                        if proc.state == ProcessState::Running {
                            return Some(current);
                        }
                    }
                }
            }
        }
        
        // Need to switch - get next process
        if let Some(next_pid) = self.ready_queue.pop_front() {
            let table = process_table().lock();
            if let Some(proc) = table.processes.iter().flatten().find(|p| p.pid == next_pid) {
                if proc.state == ProcessState::Ready {
                    self.current_time_slice = 10; // Default time slice
                    self.ready_queue.push_back(next_pid); // Re-add to queue
                    return Some(next_pid);
                }
            }
        }
        
        None
    }
    
    pub fn yield_current(&mut self) {
        if let Some(current) = get_current_pid() {
            // Move to back of queue
            self.ready_queue.retain(|&p| p != current);
            self.ready_queue.push_back(current);
            self.current_time_slice = 0;
        }
    }
}

static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

pub fn schedule() -> Option<Pid> {
    SCHEDULER.lock().schedule()
}

pub fn add_to_scheduler(pid: Pid) {
    SCHEDULER.lock().add_process(pid);
}

pub fn remove_from_scheduler(pid: Pid) {
    SCHEDULER.lock().remove_process(pid);
}

pub fn yield_cpu() {
    SCHEDULER.lock().yield_current();
}