// MSR (Model Specific Register) access utilities

pub const IA32_APIC_BASE_MSR: u32 = 0x1B;
pub const IA32_X2APIC_APICID: u32 = 0x802;

// SYSCALL/SYSRET MSRs
pub const IA32_STAR: u32 = 0xC0000081;       // Segment selectors
pub const IA32_LSTAR: u32 = 0xC0000082;      // Long mode SYSCALL entry
pub const IA32_CSTAR: u32 = 0xC0000083;      // Compatibility mode (unused)
pub const IA32_FMASK: u32 = 0xC0000084;      // RFLAGS mask

/// Read from a Model Specific Register
#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let (high, low): (u32, u32);
    core::arch::asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low,
        out("edx") high,
        options(nomem, nostack)
    );
    ((high as u64) << 32) | (low as u64)
}

/// Write to a Model Specific Register
#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    core::arch::asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
        options(nomem, nostack)
    );
}
