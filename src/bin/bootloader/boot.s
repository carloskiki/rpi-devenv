.section ".text.boot"

.globl _start
// Let's not use r0, r1, r2, for now, I think they hold useful values such as atags, and other stuff.
_start:
    // Set the stack pointer.
    mov sp, #0x8000

    // Relocate ourselves to __relocate_address
    // The size of the bootloader is __end_data - __start_text.
relocate:
    // All of these generate PC-relative instructions, since values defined in the
    // linker script evaluate to addresses in the final binary.
    ldr r3, =__physical_load_address
    ldr r4, =__relocate_address
    ldr r5, =__data_end

1:
    ldmia r3!, {r6, r7, r8, r9}
    stmia r4!, {r6, r7, r8, r9}

    // Check if we're done relocating.
    cmp r4, r5
    blo 1b

    // Zero out the BSS section.
zero_bss:
    ldr r3, =__bss_start
    ldr r4, =__bss_end
    mov r5, #0

1:
    str r5, [r3], #4
    cmp r3, r4
    blo 1b
    
    // Call into Rust.
    b first_stage

.globl mem_barrier

/**
 * @fn void dmb(void)
 *
 * Executes a data memory barrier operation using the c7 (Cache Operations)
 * register of system control coprocessor CP15.
 *
 * All explicit memory accesses occurring in program order before this operation
 * will be globally observed before any memory accesses occurring in program
 * order after this operation.  This includes both read and write accesses.
 *
 * This differs from a "data synchronization barrier" in that a data
 * synchronization barrier will ensure that all previous explicit memory
 * accesses occurring in program order have fully completed before continuing
 * and that no subsequent instructions will be executed until that point, even
 * if they do not access memory.  This is unnecessary for what we need this for.
 *
 * On the BCM2835 (Raspberry Pi), this is needed before and after accessing
 * peripherals, as documented on page 7 of the "BCM2835 ARM Peripherals"
 * document.  As documented, it is only needed when switching between
 * _different_ peripherals.
 *
    * TODO: When confident rewrite this in inline asm!.
 */
mem_barrier:
	mov	r12, #0
	mcr	p15, 0, r12, c7, c10, 5
	mov 	pc, lr
