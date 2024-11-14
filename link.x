__physical_load_address = 0x8000;

ENTRY(__physical_load_address)
SECTIONS
{
    /* Starts at load address. */
    . = __physical_load_address;
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
