/* Linker script for Angzarr kernel */

ENTRY(_start)

SECTIONS
{
    /* Start at 1MB (standard kernel load address) */
    . = 1M;

    /* Multiboot header must be at the beginning */
    .boot :
    {
        /* Ensure the multiboot header is at the beginning */
        *(.multiboot)
    }

    /* Read-only code and data */
    .text :
    {
        *(.text .text.*)
    }

    .rodata :
    {
        *(.rodata .rodata.*)
    }

    /* Read-write data */
    .data :
    {
        *(.data .data.*)
    }

    /* BSS (uninitialized data) */
    .bss :
    {
        *(.bss .bss.*)
        *(COMMON)
    }

    /* Stack (grows downward) */
    .stack :
    {
        . = ALIGN(16);
        . += 0x10000; /* 64KB stack */
        stack_top = .;
    }
}
