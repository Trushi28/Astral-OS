/* Astral OS - Limine Kernel Linker Script */
ENTRY(_start)

/* Limine loads kernel at higher half with HHDM */
KERNEL_VIRT = 0xFFFFFFFF80000000;

SECTIONS
{
    /* Start at higher half */
    . = KERNEL_VIRT;
    
    _kernel_start = .;

    /* Limine requests must be in a specific section */
    .requests : {
        KEEP(*(.requests_start_marker))
        KEEP(*(.requests))
        KEEP(*(.requests_end_marker))
    }

    /* Text/code section */
    .text ALIGN(4K) : {
        KEEP(*(.text._start))    /* Entry point first */
        *(.text .text.*)
    }

    /* Read-only data */
    .rodata ALIGN(4K) : {
        *(.rodata .rodata.*)
    }

    /* Initialized data */
    .data ALIGN(4K) : {
        *(.data .data.*)
    }

    /* Uninitialized data */
    .bss ALIGN(4K) : {
        *(COMMON)
        *(.bss .bss.*)
    }

    _kernel_end = .;

    /* Discard unnecessary sections */
    /DISCARD/ : {
        *(.eh_frame*)
        *(.note*)
        *(.comment)
        *(.debug*)
    }
}