__binary_start = 0x8000;

ENTRY(__binary_start)
SECTIONS
{
    /* Starts at LOADER_ADDR. */
    . = __binary_start;
    .text :
    {
        KEEP(*(.text.boot))
        *(.text)
        *(.text.*)
    }
    . = ALIGN(4096); /* align to page size */

    .rodata :
    {
        *(.rodata)
        *(.rodata.*)
    }

    . = ALIGN(4096); /* align to page size */

    .data : 
    { 
        *(.data)
        *(.data.*) 
    }

    .bss :
    {
        *(.bss)
        *(.bss.*)
    }

    /* 
        We do not care about stack unwinding information, so we discard it.
    */
    /DISCARD/ : { *(.ARM.exidx*) }
}
