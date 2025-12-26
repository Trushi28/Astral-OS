ENTRY(_start)

PHDRS
{
    text    PT_LOAD FLAGS(5);  /* Read + Execute */
    rodata  PT_LOAD FLAGS(4);  /* Read only */
    data    PT_LOAD FLAGS(6);  /* Read + Write */
}

SECTIONS
{
    . = 0xFFFFFFFF80000000;
    
    .text : ALIGN(4K)
    {
        *(.text)
        *(.text.*)
    } :text
    
    . = ALIGN(4K);
    
    .rodata : ALIGN(4K)
    {
        *(.rodata)
        *(.rodata.*)
    } :rodata
    
    . = ALIGN(4K);
    
    .data : ALIGN(4K)
    {
        *(.data)
        *(.data.*)
    } :data
    
    .bss : ALIGN(4K)
    {
        *(.bss)
        *(.bss.*)
        *(COMMON)
    } :data
    
    /DISCARD/ :
    {
        *(.eh_frame)
        *(.note*)
    }
}
