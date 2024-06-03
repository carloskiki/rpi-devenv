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

    .rodata :
    {
        *(.rodata)
        *(.rodata.*)
    }

    .data : 
    { 
        *(.data)
        *(.data.*) 
    }

    __bss_start = .;
    .bss :
    {
        *(.bss)
        *(.bss.*)
    }
    __bss_end = .;
    
    /* 
        We do not care about stack unwinding information, so we discard it.
    */
    /DISCARD/ : { *(.ARM.exidx*) }
}
