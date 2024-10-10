.section ".text.boot"

.globl _start
_start:
    // Set the stack pointer.
    mov r0,#0x8000

    // Zero out the BSS section. This should work even if the BSS section is zero bytes.
    ldr r0, =__bss_start
    ldr r1, =__bss_end
    mov r2, #0
    bl zero_bss
    
    // Call into Rust.
    b first_stage

zero_bss:
    cmp r0, r1
    bxge lr
    str r2, [r0], #4
    b zero_bss

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
