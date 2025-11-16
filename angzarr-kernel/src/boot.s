/* Multiboot header for GRUB bootloader */
.section .multiboot
.align 4

/* Multiboot constants */
.set MULTIBOOT_MAGIC,        0x1BADB002
.set MULTIBOOT_PAGE_ALIGN,   1 << 0
.set MULTIBOOT_MEMORY_INFO,  1 << 1
.set MULTIBOOT_FLAGS,        MULTIBOOT_PAGE_ALIGN | MULTIBOOT_MEMORY_INFO
.set MULTIBOOT_CHECKSUM,     -(MULTIBOOT_MAGIC + MULTIBOOT_FLAGS)

/* Multiboot header */
multiboot_header:
    .long MULTIBOOT_MAGIC
    .long MULTIBOOT_FLAGS
    .long MULTIBOOT_CHECKSUM

.section .bss
.align 16
stack_bottom:
    .skip 16384  /* 16 KB stack */
stack_top:

.section .text
.global _start
.type _start, @function
_start:
    /* Set up stack */
    mov $stack_top, %esp

    /* Call Rust kernel */
    call rust_main

    /* If rust_main returns, hang */
hang:
    cli
    hlt
    jmp hang

.size _start, . - _start

/* Rust entry point */
.global rust_main
.type rust_main, @function
