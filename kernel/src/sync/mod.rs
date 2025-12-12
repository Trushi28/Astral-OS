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