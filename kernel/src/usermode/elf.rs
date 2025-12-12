// ============ src/usermode/elf.rs ============
pub const PT_LOAD: u32 = 1;

#[repr(C)]
pub struct Elf64Header {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

impl Elf64Header {
    pub fn parse(data: &[u8]) -> Result<&Self, &'static str> {
        if data.len() < core::mem::size_of::<Self>() {
            return Err("Data too small for ELF header");
        }
        
        unsafe {
            let header = &*(data.as_ptr() as *const Self);
            
            if &header.e_ident[0..4] != b"\x7FELF" {
                return Err("Invalid ELF magic");
            }
            
            if header.e_ident[4] != 2 {
                return Err("Not 64-bit ELF");
            }
            
            Ok(header)
        }
    }
    
    pub fn entry_point(&self) -> u64 {
        self.e_entry
    }
    
    pub fn program_headers<'a>(&self, data: &'a [u8]) -> Result<&'a [Elf64ProgramHeader], &'static str> {
        let phoff = self.e_phoff as usize;
        let phnum = self.e_phnum as usize;
        let phsize = self.e_phentsize as usize;
        
        if phoff + (phnum * phsize) > data.len() {
            return Err("Program headers out of bounds");
        }
        
        unsafe {
            let ptr = data.as_ptr().add(phoff) as *const Elf64ProgramHeader;
            Ok(core::slice::from_raw_parts(ptr, phnum))
        }
    }
}

#[repr(C)]
pub struct Elf64ProgramHeader {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

impl Elf64ProgramHeader {
    pub fn is_writable(&self) -> bool {
        (self.p_flags & 0x2) != 0
    }
    
    pub fn is_executable(&self) -> bool {
        (self.p_flags & 0x1) != 0
    }
    
    pub fn get_data<'a>(&self, elf_data: &'a [u8]) -> Option<&'a [u8]> {
        let start = self.p_offset as usize;
        let end = start + self.p_filesz as usize;
        
        if end > elf_data.len() {
            return None;
        }
        
        Some(&elf_data[start..end])
    }
}

