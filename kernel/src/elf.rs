//! ELF Parser and Loader
//!
//! Parses ELF64 binaries and loads them into user address space.

use x86_64::VirtAddr;
use crate::serial_println;
use crate::memory::user_memory;

/// ELF Magic bytes
pub const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

/// ELF class (32-bit vs 64-bit)
pub const ELFCLASS64: u8 = 2;

/// ELF data encoding (little-endian vs big-endian)
pub const ELFDATA2LSB: u8 = 1; // Little-endian

/// ELF type
pub const ET_EXEC: u16 = 2; // Executable

/// Machine type
pub const EM_X86_64: u16 = 62; // AMD x86-64

/// Program header types
pub const PT_NULL: u32 = 0;
pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;

/// Program header flags
pub const PF_X: u32 = 0x1; // Executable
pub const PF_W: u32 = 0x2; // Writable
pub const PF_R: u32 = 0x4; // Readable

/// ELF64 Header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Header {
    /// ELF identification (magic, class, endian, version, OS ABI, padding)
    pub e_ident: [u8; 16],
    /// Object file type
    pub e_type: u16,
    /// Machine type
    pub e_machine: u16,
    /// Object file version
    pub e_version: u32,
    /// Entry point virtual address
    pub e_entry: u64,
    /// Program header table file offset
    pub e_phoff: u64,
    /// Section header table file offset
    pub e_shoff: u64,
    /// Processor-specific flags
    pub e_flags: u32,
    /// ELF header size
    pub e_ehsize: u16,
    /// Program header table entry size
    pub e_phentsize: u16,
    /// Program header table entry count
    pub e_phnum: u16,
    /// Section header table entry size
    pub e_shentsize: u16,
    /// Section header table entry count
    pub e_shnum: u16,
    /// Section header string table index
    pub e_shstrndx: u16,
}

impl Elf64Header {
    /// Validate the ELF header
    pub fn validate(&self) -> Result<(), &'static str> {
        // Check magic
        if self.e_ident[0..4] != ELF_MAGIC {
            return Err("Invalid ELF magic");
        }
        
        // Check class (must be 64-bit)
        if self.e_ident[4] != ELFCLASS64 {
            return Err("Not a 64-bit ELF");
        }
        
        // Check endianness (must be little-endian)
        if self.e_ident[5] != ELFDATA2LSB {
            return Err("Not little-endian");
        }
        
        // Check machine type (must be x86-64)
        if self.e_machine != EM_X86_64 {
            return Err("Not x86-64 architecture");
        }
        
        Ok(())
    }
    
    /// Get entry point
    pub fn entry_point(&self) -> VirtAddr {
        VirtAddr::new(self.e_entry)
    }
}

/// ELF64 Program Header
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64ProgramHeader {
    /// Segment type
    pub p_type: u32,
    /// Segment flags
    pub p_flags: u32,
    /// Segment file offset
    pub p_offset: u64,
    /// Segment virtual address
    pub p_vaddr: u64,
    /// Segment physical address (unused on x86-64)
    pub p_paddr: u64,
    /// Segment size in file
    pub p_filesz: u64,
    /// Segment size in memory
    pub p_memsz: u64,
    /// Segment alignment
    pub p_align: u64,
}

impl Elf64ProgramHeader {
    /// Check if this is a loadable segment
    pub fn is_load(&self) -> bool {
        self.p_type == PT_LOAD
    }
    
    /// Check if segment is executable
    pub fn is_executable(&self) -> bool {
        (self.p_flags & PF_X) != 0
    }
    
    /// Check if segment is writable
    pub fn is_writable(&self) -> bool {
        (self.p_flags & PF_W) != 0
    }
    
    /// Check if segment is readable
    pub fn is_readable(&self) -> bool {
        (self.p_flags & PF_R) != 0
    }
}

/// Parsed ELF file
pub struct Elf<'a> {
    /// Raw ELF data
    data: &'a [u8],
    /// Parsed header
    pub header: &'a Elf64Header,
}

impl<'a> Elf<'a> {
    /// Parse an ELF file from raw bytes
    pub fn parse(data: &'a [u8]) -> Result<Self, &'static str> {
        if data.len() < core::mem::size_of::<Elf64Header>() {
            return Err("ELF data too small for header");
        }
        
        let header = unsafe { &*(data.as_ptr() as *const Elf64Header) };
        header.validate()?;
        
        // Copy fields to avoid unaligned references
        let entry = header.e_entry;
        let phnum = header.e_phnum;
        let phoff = header.e_phoff;
        
        serial_println!("[ELF] Valid ELF64 binary detected");
        serial_println!("[ELF]   Entry point: {:#x}", entry);
        serial_println!("[ELF]   Program headers: {} at offset {:#x}", phnum, phoff);
        
        Ok(Self { data, header })
    }
    
    /// Get program headers iterator
    pub fn program_headers(&self) -> impl Iterator<Item = &Elf64ProgramHeader> {
        let phoff = self.header.e_phoff as usize;
        let phnum = self.header.e_phnum as usize;
        let phentsize = self.header.e_phentsize as usize;
        
        (0..phnum).map(move |i| {
            let offset = phoff + i * phentsize;
            unsafe { &*(self.data.as_ptr().add(offset) as *const Elf64ProgramHeader) }
        })
    }
    
    /// Get loadable segments
    pub fn load_segments(&self) -> impl Iterator<Item = &Elf64ProgramHeader> {
        self.program_headers().filter(|ph| ph.is_load())
    }
    
    /// Get segment data
    pub fn segment_data(&self, ph: &Elf64ProgramHeader) -> &[u8] {
        let offset = ph.p_offset as usize;
        let size = ph.p_filesz as usize;
        &self.data[offset..offset + size]
    }
}

/// Load an ELF binary into user address space
/// 
/// Returns the entry point address
pub fn load_elf(elf_data: &[u8]) -> Result<VirtAddr, &'static str> {
    let elf = Elf::parse(elf_data)?;
    
    serial_println!("[ELF] Loading segments...");
    
    for ph in elf.load_segments() {
        let vaddr = VirtAddr::new(ph.p_vaddr);
        let memsz = ph.p_memsz;
        let filesz = ph.p_filesz;
        
        serial_println!("[ELF]   Segment: vaddr={:#x}, memsz={:#x}, filesz={:#x}", 
            vaddr.as_u64(), memsz, filesz);
        serial_println!("[ELF]     Flags: R={} W={} X={}", 
            ph.is_readable(), ph.is_writable(), ph.is_executable());
        
        // Map the segment into user memory
        // NOTE: We always map as writable initially so we can copy data.
        // A proper implementation would change permissions after copying.
        user_memory::map_user_region(
            vaddr,
            memsz,
            true, // Always writable for initial copy
            ph.is_executable(),
        )?;
        
        // Copy segment data
        let segment_data = elf.segment_data(ph);
        let dest_ptr = vaddr.as_u64() as *mut u8;
        
        unsafe {
            // Copy file contents
            core::ptr::copy_nonoverlapping(
                segment_data.as_ptr(),
                dest_ptr,
                filesz as usize,
            );
            
            // Zero-fill .bss (memsz > filesz)
            if memsz > filesz {
                let bss_start = dest_ptr.add(filesz as usize);
                let bss_size = (memsz - filesz) as usize;
                core::ptr::write_bytes(bss_start, 0, bss_size);
            }
        }
        
        serial_println!("[ELF]     Loaded {} bytes, zeroed {} bytes", 
            filesz, memsz.saturating_sub(filesz));
    }
    
    Ok(elf.header.entry_point())
}

/// Load and execute an ELF binary
/// 
/// This is the main entry point for running user programs.
/// It creates a user address space, loads the ELF, allocates a stack,
/// and jumps to Ring 3.
pub unsafe fn exec_elf(elf_data: &[u8]) -> Result<!, &'static str> {
    use crate::arch::x86_64::usermode;
    
    serial_println!("[ELF] === Starting ELF execution ===");
    serial_println!("[ELF] DEBUG: Step 1 - Creating user page table...");
    
    // Create user page table
    let (user_cr3, _pml4_virt) = user_memory::create_user_page_table()?;
    serial_println!("[ELF] DEBUG: Step 2 - PML4 created at {:#x}", user_cr3);
    
    // Switch to user page table
    serial_println!("[ELF] DEBUG: Step 3 - Switching CR3...");
    x86_64::registers::control::Cr3::write(
        x86_64::structures::paging::PhysFrame::containing_address(
            x86_64::PhysAddr::new(user_cr3)
        ),
        x86_64::registers::control::Cr3Flags::empty()
    );
    serial_println!("[ELF] DEBUG: Step 4 - CR3 switched OK");
    
    // Load ELF segments
    serial_println!("[ELF] DEBUG: Step 5 - Loading ELF segments...");
    let entry_point = load_elf(elf_data)?;
    serial_println!("[ELF] DEBUG: Step 6 - Entry point: {:#x}", entry_point.as_u64());
    
    // Allocate user stack
    serial_println!("[ELF] DEBUG: Step 7 - Allocating user stack...");
    let user_stack = user_memory::allocate_user_stack()?;
    serial_println!("[ELF] DEBUG: Step 8 - Stack allocated at {:#x}", user_stack.as_u64());
    
    serial_println!("[ELF] DEBUG: Step 9 - About to jump to Ring 3");
    serial_println!("[ELF] Entry: {:#x}, Stack: {:#x}", entry_point.as_u64(), user_stack.as_u64());
    
    // Jump to usermode!
    usermode::jump_to_usermode(entry_point, user_stack)
}

