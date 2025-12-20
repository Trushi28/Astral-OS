/* Userland Linker Script - loads at fixed address 0x400000 */
OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

SECTIONS
{
    /* User code starts at 4MB - standard user base address */
    . = 0x400000;

    .text : {
        *(.text._start)    /* Entry point first */
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        *(.bss .bss.*)
    }

    /* Discard debug and unnecessary sections */
    /DISCARD/ : {
        *(.eh_frame)
        *(.note*)
        *(.comment)
    }
}
