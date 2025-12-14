// src/arch/x86_64/mod.rs
//! x86_64 architecture support

pub mod cpu;
pub mod apic;
pub mod ioapic;
pub mod smp;
pub mod tss;
pub mod gdt;
pub mod syscall;
pub mod usermode;

pub use cpu::{CpuId, get_cpu_id, get_cpu_count, CpuInfo};
pub use apic::{init_apic, local_apic_eoi, send_ipi, ApicId, IpiDestination};
pub use ioapic::{init_ioapic, ioapic_set_irq};
pub use smp::init_smp;