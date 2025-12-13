// src/sync/mod.rs
//! Synchronization primitives for SMP

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::arch::asm;

/// Spinlock with IRQ disable
pub struct IrqSpinlock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for IrqSpinlock<T> {}
unsafe impl<T: Send> Send for IrqSpinlock<T> {}

impl<T> IrqSpinlock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }
    
    pub fn lock(&self) -> IrqSpinlockGuard<T> {
        // Save interrupt flag and disable interrupts
        let flags = self.save_and_disable_irq();
        
        // Spin until we acquire the lock
        while self.locked.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed
        ).is_err() {
            // Hint to CPU that we're spinning
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        
        IrqSpinlockGuard {
            lock: self,
            flags,
        }
    }
    
    pub fn try_lock(&self) -> Option<IrqSpinlockGuard<T>> {
        let flags = self.save_and_disable_irq();
        
        if self.locked.compare_exchange(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed
        ).is_ok() {
            Some(IrqSpinlockGuard {
                lock: self,
                flags,
            })
        } else {
            self.restore_irq(flags);
            None
        }
    }
    
    fn save_and_disable_irq(&self) -> u64 {
        let flags: u64;
        unsafe {
            asm!(
                "pushfq",
                "pop {flags}",
                "cli",
                flags = out(reg) flags,
                options(nomem, preserves_flags)
            );
        }
        flags
    }
    
    fn restore_irq(&self, flags: u64) {
        unsafe {
            if flags & 0x200 != 0 {
                asm!("sti", options(nomem, nostack));
            }
        }
    }
}

pub struct IrqSpinlockGuard<'a, T> {
    lock: &'a IrqSpinlock<T>,
    flags: u64,
}

impl<'a, T> Drop for IrqSpinlockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
        self.lock.restore_irq(self.flags);
    }
}

impl<'a, T> Deref for IrqSpinlockGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for IrqSpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

/// Read-Write lock (readers-writer lock)
pub struct RwLock<T> {
    readers: AtomicUsize,
    writer: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for RwLock<T> {}
unsafe impl<T: Send> Send for RwLock<T> {}

impl<T> RwLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            readers: AtomicUsize::new(0),
            writer: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }
    
    pub fn read(&self) -> RwLockReadGuard<T> {
        loop {
            // Wait for writer to finish
            while self.writer.load(Ordering::Acquire) {
                core::hint::spin_loop();
            }
            
            // Increment reader count
            self.readers.fetch_add(1, Ordering::Acquire);
            
            // Check no writer sneaked in
            if !self.writer.load(Ordering::Acquire) {
                return RwLockReadGuard { lock: self };
            }
            
            // Writer appeared, back off
            self.readers.fetch_sub(1, Ordering::Release);
        }
    }
    
    pub fn write(&self) -> RwLockWriteGuard<T> {
        // Acquire writer lock
        while self.writer.compare_exchange_weak(
            false,
            true,
            Ordering::Acquire,
            Ordering::Relaxed
        ).is_err() {
            while self.writer.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        
        // Wait for all readers to finish
        while self.readers.load(Ordering::Acquire) != 0 {
            core::hint::spin_loop();
        }
        
        RwLockWriteGuard { lock: self }
    }
}

pub struct RwLockReadGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<'a, T> Drop for RwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.readers.fetch_sub(1, Ordering::Release);
    }
}

impl<'a, T> Deref for RwLockReadGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

pub struct RwLockWriteGuard<'a, T> {
    lock: &'a RwLock<T>,
}

impl<'a, T> Drop for RwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.writer.store(false, Ordering::Release);
    }
}

impl<'a, T> Deref for RwLockWriteGuard<'a, T> {
    type Target = T;
    
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T> DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

/// Per-CPU variable
pub struct PerCpu<T> {
    data: [UnsafeCell<T>; crate::arch::cpu::MAX_CPUS],
}

unsafe impl<T: Send> Sync for PerCpu<T> {}
unsafe impl<T: Send> Send for PerCpu<T> {}

impl<T: Copy> PerCpu<T> {
    pub const fn new(init: T) -> Self {
        // UnsafeCell doesn't implement Copy, so we need manual initialization
        // Use a const loop with pointer writes
        unsafe {
            // Create uninitialized memory
            let mut result = core::mem::MaybeUninit::<Self>::uninit();
            let data_ptr = core::ptr::addr_of_mut!((*result.as_mut_ptr()).data);
            
            // Initialize each element
            let mut i = 0;
            while i < crate::arch::cpu::MAX_CPUS {
                let elem_ptr = (data_ptr as *mut UnsafeCell<T>).add(i);
                core::ptr::write(elem_ptr, UnsafeCell::new(init));
                i += 1;
            }
            
            result.assume_init()
        }
    }
    
    pub fn get(&self) -> &T {
        let cpu_id = crate::arch::cpu::get_cpu_id();
        unsafe { &*self.data[cpu_id.as_usize()].get() }
    }
    
    pub fn get_mut(&self) -> &mut T {
        let cpu_id = crate::arch::cpu::get_cpu_id();
        unsafe { &mut *self.data[cpu_id.as_usize()].get() }
    }
    
    pub fn get_for_cpu(&self, cpu_id: crate::arch::cpu::CpuId) -> &T {
        unsafe { &*self.data[cpu_id.as_usize()].get() }
    }
}

/// Memory barrier
#[inline(always)]
pub fn memory_barrier() {
    core::sync::atomic::compiler_fence(Ordering::SeqCst);
    unsafe {
        asm!("mfence", options(nostack, preserves_flags));
    }
}

/// Compiler barrier
#[inline(always)]
pub fn compiler_barrier() {
    core::sync::atomic::compiler_fence(Ordering::SeqCst);
}

// ============================================================================
// Additional Synchronization Primitives
// ============================================================================

/// Counting semaphore
pub struct Semaphore {
    count: AtomicUsize,
    max_count: usize,
}

impl Semaphore {
    /// Create a semaphore with initial count
    pub const fn new(initial: usize) -> Self {
        Self {
            count: AtomicUsize::new(initial),
            max_count: usize::MAX,
        }
    }
    
    /// Create a bounded semaphore
    pub const fn bounded(initial: usize, max: usize) -> Self {
        Self {
            count: AtomicUsize::new(initial),
            max_count: max,
        }
    }
    
    /// Acquire (decrement) - blocks if count is 0
    pub fn acquire(&self) {
        loop {
            let current = self.count.load(Ordering::Acquire);
            if current > 0 {
                if self.count.compare_exchange_weak(
                    current,
                    current - 1,
                    Ordering::AcqRel,
                    Ordering::Relaxed
                ).is_ok() {
                    return;
                }
            }
            core::hint::spin_loop();
        }
    }
    
    /// Try to acquire without blocking
    pub fn try_acquire(&self) -> bool {
        let current = self.count.load(Ordering::Acquire);
        if current > 0 {
            self.count.compare_exchange(
                current,
                current - 1,
                Ordering::AcqRel,
                Ordering::Relaxed
            ).is_ok()
        } else {
            false
        }
    }
    
    /// Release (increment)
    pub fn release(&self) {
        loop {
            let current = self.count.load(Ordering::Acquire);
            if current >= self.max_count {
                return; // At max
            }
            if self.count.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Relaxed
            ).is_ok() {
                return;
            }
        }
    }
    
    /// Get current count
    pub fn available(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Condition variable (busy-wait version)
pub struct Condvar {
    notified: AtomicBool,
    waiters: AtomicUsize,
}

impl Condvar {
    pub const fn new() -> Self {
        Self {
            notified: AtomicBool::new(false),
            waiters: AtomicUsize::new(0),
        }
    }
    
    /// Wait for a notification (busy-wait)
    pub fn wait(&self) {
        self.waiters.fetch_add(1, Ordering::Relaxed);
        
        while !self.notified.swap(false, Ordering::Acquire) {
            core::hint::spin_loop();
        }
        
        self.waiters.fetch_sub(1, Ordering::Relaxed);
    }
    
    /// Notify one waiter
    pub fn notify_one(&self) {
        self.notified.store(true, Ordering::Release);
    }
    
    /// Notify all waiters
    pub fn notify_all(&self) {
        // Set flag multiple times for all waiters
        let count = self.waiters.load(Ordering::Relaxed);
        for _ in 0..count {
            self.notified.store(true, Ordering::Release);
        }
    }
    
    /// Check if there are waiters
    pub fn has_waiters(&self) -> bool {
        self.waiters.load(Ordering::Relaxed) > 0
    }
}

/// Barrier for synchronizing multiple threads
pub struct Barrier {
    threshold: usize,
    count: AtomicUsize,
    generation: AtomicUsize,
}

impl Barrier {
    /// Create a barrier for n threads
    pub const fn new(n: usize) -> Self {
        Self {
            threshold: n,
            count: AtomicUsize::new(0),
            generation: AtomicUsize::new(0),
        }
    }
    
    /// Wait at the barrier
    pub fn wait(&self) {
        let gen = self.generation.load(Ordering::Relaxed);
        let count = self.count.fetch_add(1, Ordering::AcqRel);
        
        if count + 1 >= self.threshold {
            // Last thread to arrive - reset and advance generation
            self.count.store(0, Ordering::Relaxed);
            self.generation.fetch_add(1, Ordering::Release);
        } else {
            // Wait for generation to change
            while self.generation.load(Ordering::Acquire) == gen {
                core::hint::spin_loop();
            }
        }
    }
}

/// Once - run initialization exactly once
pub struct Once {
    state: AtomicUsize,
}

const ONCE_UNINIT: usize = 0;
const ONCE_RUNNING: usize = 1;
const ONCE_COMPLETE: usize = 2;

impl Once {
    pub const fn new() -> Self {
        Self {
            state: AtomicUsize::new(ONCE_UNINIT),
        }
    }
    
    /// Call the function exactly once
    pub fn call_once<F: FnOnce()>(&self, f: F) {
        if self.state.load(Ordering::Acquire) == ONCE_COMPLETE {
            return;
        }
        
        if self.state.compare_exchange(
            ONCE_UNINIT,
            ONCE_RUNNING,
            Ordering::AcqRel,
            Ordering::Acquire
        ).is_ok() {
            f();
            self.state.store(ONCE_COMPLETE, Ordering::Release);
        } else {
            // Wait for completion
            while self.state.load(Ordering::Acquire) != ONCE_COMPLETE {
                core::hint::spin_loop();
            }
        }
    }
    
    /// Check if initialization is complete
    pub fn is_complete(&self) -> bool {
        self.state.load(Ordering::Acquire) == ONCE_COMPLETE
    }
}