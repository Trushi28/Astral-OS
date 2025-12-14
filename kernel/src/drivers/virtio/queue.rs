//src/drivers/virtio/queue.rs
use crate::util::align_up;
use crate::PAGE_SIZE;
use core::sync::atomic::{fence, Ordering};

const QUEUE_SIZE: u16 = 128;

const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;
const VIRTQ_AVAIL_F_NO_INTERRUPT: u16 = 1;

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug)]
pub struct VirtqDesc {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

impl VirtqDesc {
    pub const fn new() -> Self {
        Self {
            addr: 0,
            len: 0,
            flags: 0,
            next: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtqAvail {
    pub flags: u16,
    pub idx: u16,
    pub ring: [u16; QUEUE_SIZE as usize],
    pub used_event: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VirtqUsedElem {
    pub id: u32,
    pub len: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VirtqUsed {
    pub flags: u16,
    pub idx: u16,
    pub ring: [VirtqUsedElem; QUEUE_SIZE as usize],
    pub avail_event: u16,
}

pub struct Virtqueue {
    queue_size: u16,
    
    desc: *mut [VirtqDesc; QUEUE_SIZE as usize],
    avail: *mut VirtqAvail,
    pub used: *mut VirtqUsed,
    
    free_desc: [bool; QUEUE_SIZE as usize],
    
    pub last_used_idx: u16,
    next_avail_idx: u16,
    
    desc_phys: u64,
    avail_phys: u64,
    used_phys: u64,
}

unsafe impl Send for Virtqueue {}
unsafe impl Sync for Virtqueue {}

impl Virtqueue {
    pub fn size_requirements(queue_size: u16) -> (usize, usize, usize) {
        let desc_size = 16 * queue_size as usize;
        let avail_size = 6 + (2 * queue_size as usize);
        let _used_size = 6 + (8 * queue_size as usize);
        
        let avail_offset = desc_size;
        let used_offset = align_up(avail_offset + avail_size, 4);
        
        (desc_size, avail_offset, used_offset)
    }
    
    pub unsafe fn new(base_virt: usize, base_phys: u64, queue_size: u16) -> Result<Self, &'static str> {
        if !queue_size.is_power_of_two() || queue_size > QUEUE_SIZE {
            return Err("Invalid queue size");
        }
        
        let (_desc_size, avail_offset, used_offset) = Self::size_requirements(queue_size);
        
        let total_required = used_offset + 6 + (8 * queue_size as usize);
        if total_required > PAGE_SIZE {
            return Err("Queue too large for one page");
        }
        
        let desc = base_virt as *mut [VirtqDesc; QUEUE_SIZE as usize];
        let avail = (base_virt + avail_offset) as *mut VirtqAvail;
        let used = (base_virt + used_offset) as *mut VirtqUsed;
        
        let desc_phys = base_phys;
        let avail_phys = base_phys + avail_offset as u64;
        let used_phys = base_phys + used_offset as u64;
        
        core::ptr::write_bytes(desc, 0, 1);
        core::ptr::write_bytes(avail, 0, 1);
        core::ptr::write_bytes(used, 0, 1);
        
        (*avail).flags = VIRTQ_AVAIL_F_NO_INTERRUPT;

        let mut free_desc = [false; QUEUE_SIZE as usize];
        for i in 0..queue_size as usize {
            free_desc[i] = true;
        }
        
        Ok(Self {
            queue_size,
            desc,
            avail,
            used,
            free_desc,
            last_used_idx: 0,
            next_avail_idx: 0,
            desc_phys,
            avail_phys,
            used_phys,
        })
    }
    
    pub fn alloc_desc(&mut self) -> Option<u16> {
        for i in 0..self.queue_size {
            if self.free_desc[i as usize] {
                self.free_desc[i as usize] = false;
                return Some(i);
            }
        }
        None
    }
    
    pub fn free_desc(&mut self, idx: u16) {
        if idx < self.queue_size {
            self.free_desc[idx as usize] = true;
        }
    }
    
    pub fn add_buf(
        &mut self,
        head_idx: u16,
        buffers: &[(u64, u32, bool)],
    ) -> Result<(), &'static str> {
        if buffers.is_empty() {
            return Err("Empty buffer list");
        }
        
        for &(addr, len, _) in buffers.iter() {
            if len == 0 || addr == 0 {
                return Err("Invalid buffer");
            }
        }
        
        unsafe {
            let desc_table = &mut *self.desc;
            
            let mut desc_indices = alloc::vec::Vec::with_capacity(buffers.len());
            desc_indices.push(head_idx);
            
            for _i in 1..buffers.len() {
                match self.alloc_desc() {
                    Some(idx) => desc_indices.push(idx),
                    None => {
                        for &idx in &desc_indices[1..] {
                            self.free_desc(idx);
                        }
                        return Err("Out of descriptors");
                    }
                }
            }
            
            for (i, &(addr, len, writable)) in buffers.iter().enumerate() {
                let desc_idx = desc_indices[i] as usize;
                
                desc_table[desc_idx].addr = addr;
                desc_table[desc_idx].len = len;
                
                let mut flags = 0u16;
                if writable {
                    flags |= VIRTQ_DESC_F_WRITE;
                }
                if i < buffers.len() - 1 {
                    flags |= VIRTQ_DESC_F_NEXT;
                    desc_table[desc_idx].next = desc_indices[i + 1];
                } else {
                    desc_table[desc_idx].next = 0;
                }
                
                desc_table[desc_idx].flags = flags;
            }
            
            fence(Ordering::Release);
            
            let avail = &mut *self.avail;
            let avail_idx = self.next_avail_idx % QUEUE_SIZE;
            avail.ring[avail_idx as usize] = head_idx;
            
            fence(Ordering::Release);
            
            avail.idx = avail.idx.wrapping_add(1);
            self.next_avail_idx = self.next_avail_idx.wrapping_add(1);
            
            fence(Ordering::SeqCst);
        }
        
        Ok(())
    }
    
    pub fn get_used(&mut self) -> Option<(u16, u32)> {
        unsafe {
            let used = &*self.used;
            
            fence(Ordering::Acquire);
            
            if self.last_used_idx == used.idx {
                return None;
            }
            
            let idx = self.last_used_idx as usize % QUEUE_SIZE as usize;
            let elem = used.ring[idx];
            
            self.last_used_idx = self.last_used_idx.wrapping_add(1);
            
            let mut desc_idx = elem.id as u16;
            loop {
                let desc = &(*self.desc)[desc_idx as usize];
                let next = desc.next;
                let has_next = (desc.flags & VIRTQ_DESC_F_NEXT) != 0;
                
                self.free_desc(desc_idx);
                
                if !has_next {
                    break;
                }
                desc_idx = next;
            }
            
            Some((elem.id as u16, elem.len))
        }
    }
    
    pub fn get_addresses(&self) -> (u64, u64, u64) {
        (self.desc_phys, self.avail_phys, self.used_phys)
    }
    
    pub fn size(&self) -> u16 {
        self.queue_size
    }
}