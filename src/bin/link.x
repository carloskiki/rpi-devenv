/* The linker script for the uart bootloader of the Raspberry Pi 0. */

/* We do not care about setting a LOAD address, because the raspberry pi always loads the binary at 0x8000. */
__physical_load_address = 0x8000;

/* Arbitrary place to relocate ourselves to. We want to get out of the way, and load a kernel that we
   receive over UART at the __physical_load_address. */
__relocate_address = 0x2000000;

ENTRY(__physical_load_address)
SECTIONS
{
    /* Starts at LOADER_ADDR. */
    . = __relocate_address;
    __text_start = .;
    .text :
    {
        KEEP(*(.text.boot))
        *(.text)
        *(.text.*)
    }
    __text_end = .;

    __rodata_start = .;
    .rodata :
    {
        *(.rodata)
        *(.rodata.*)
    }
    __rodata_end = .;

    __data_start = .;
    .data : 
    { 
        *(.data)
        *(.data.*) 
    }
    __data_end = .;

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
