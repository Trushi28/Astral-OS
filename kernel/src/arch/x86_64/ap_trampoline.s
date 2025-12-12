# AP Trampoline - Code that runs to bring up Application Processors
# This code is copied to physical address 0x8000 and executed in real mode
# It transitions: 16-bit real -> 32-bit protected -> 64-bit long mode

.section .rodata

.global ap_trampoline_start
.global ap_trampoline_end

.align 16
ap_trampoline_start:

# ============ 16-bit Real Mode Entry Point ============
.code16

ap_start_16:
    cli
    cld
    
    # Set up segments
    xorw %ax, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    
    # Load the GDT - it's at a fixed offset from our base
    movw $0x8000, %bx
    addw $(trampoline_gdt_ptr - ap_trampoline_start), %bx
    lgdt (%bx)
    
    # Enable protected mode (CR0.PE = 1)
    movl %cr0, %eax
    orl $0x1, %eax
    movl %eax, %cr0
    
    # Far jump to 32-bit code - this flushes the pipeline
    ljmpl $0x08, $(ap_start_32 - ap_trampoline_start + 0x8000)

# ============ 32-bit Protected Mode ============
.code32
.align 8
ap_start_32:
    # Set up 32-bit data segments
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %fs
    movw %ax, %gs
    movw %ax, %ss
    
    # Enable PAE (required for long mode)
    movl %cr4, %eax
    orl $0x20, %eax
    movl %eax, %cr4
    
    # Load CR3 with the PML4 address from parameter area
    # Parameters are at 0x8500 (0x8000 + 0x500)
    # Offset 16 = CR3 (page table)
    movl 0x8510, %eax
    movl %eax, %cr3
    
    # Enable long mode via IA32_EFER MSR
    movl $0xC0000080, %ecx
    rdmsr
    orl $0x100, %eax  # Set LME (Long Mode Enable) bit
    wrmsr
    
    # Enable paging (CR0.PG = 1)
    movl %cr0, %eax
    orl $0x80000000, %eax
    movl %eax, %cr0
    
    # Far jump to 64-bit code 
    ljmpl $0x18, $(ap_start_64 - ap_trampoline_start + 0x8000)

# ============ 64-bit Long Mode ============
.code64
.align 8
ap_start_64:
    # Set up 64-bit data segments
    movw $0x20, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    xorw %ax, %ax
    movw %ax, %fs
    movw %ax, %gs
    
    # Load stack pointer from parameter area FIRST
    # Offset 0 = stack pointer (must be 16-byte aligned)
    movq 0x8500, %rsp
    
    # Align stack to 16 bytes
    andq $-16, %rsp
    
    # Load the kernel's GDT from inline descriptor
    # Offset 24 = kernel GDT limit (2 bytes)
    # Offset 26 = kernel GDT base (8 bytes)
    leaq kernel_gdt_desc(%rip), %rax
    movw 0x8518, %cx           # Load limit
    movw %cx, (%rax)
    movq 0x851A, %rcx          # Load base
    movq %rcx, 2(%rax)
    lgdt (%rax)
    
    # Reload code segment with kernel GDT
    # Use far return trick to reload CS
    pushq $0x08               # Kernel code segment
    leaq .reload_cs(%rip), %rax
    pushq %rax
    lretq
    
.reload_cs:
    # Reload data segments with kernel GDT values
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    xorw %ax, %ax
    movw %ax, %fs
    movw %ax, %gs
    
    # Set GS base for per-CPU data
    # Offset 40 = GS base
    movq 0x8528, %rax
    movq %rax, %rdx
    shrq $32, %rdx
    movl $0xC0000101, %ecx  # IA32_GS_BASE
    wrmsr
    
    # Load entry point and jump to it
    # Offset 8 = entry point
    movq 0x8508, %rax
    
    # Ensure stack is 16-byte aligned before call
    # (subtract 8 because call pushes return address)
    subq $8, %rsp
    
    jmpq *%rax

# ============ GDT for Trampoline ============
.align 16
trampoline_gdt:
    .quad 0x0000000000000000  # 0x00: Null descriptor
    .quad 0x00CF9A000000FFFF  # 0x08: 32-bit code segment
    .quad 0x00CF92000000FFFF  # 0x10: 32-bit data segment
    .quad 0x00209A0000000000  # 0x18: 64-bit code (L=1, D=0)
    .quad 0x0000920000000000  # 0x20: 64-bit data
trampoline_gdt_end:

.align 8
trampoline_gdt_ptr:
    .word trampoline_gdt_end - trampoline_gdt - 1
    .long trampoline_gdt - ap_trampoline_start + 0x8000

# Temporary storage for kernel GDT descriptor
.align 8
kernel_gdt_desc:
    .word 0
    .quad 0

.align 16
ap_trampoline_end: