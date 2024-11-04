.section ".text.boot"

.globl _start

_start:
    // Trick from dwelch67: https://github.com/dwelch67/raspberrypi-zero/tree/master/blinker05
    ldr pc, reset_handler
    ldr pc, undefined_handler
    ldr pc, swi_handler
    ldr pc, prefetch_handler
    ldr pc, data_handler
    ldr pc, unused_handler
    ldr pc, irq_handler
    ldr pc, fiq_handler
reset_handler:      .word reset
undefined_handler:  .word abort_with_panic
swi_handler:        .word isr
prefetch_handler:   .word abort_with_panic
data_handler:       .word abort_with_panic
unused_handler:     .word hang
irq_handler:        .word isr
fiq_handler:        .word hang

// Let's not use r0, r1, r2, for now, I think they hold useful values such as atags, and other stuff.
reset:
    // Setup the interrupt vector table.
    ldr r3, =__physical_load_address
    mov r4, #0 // At address 0x00000000
    ldmia r3!,{{r5,r6,r7,r8,r9,r10,r11,r12}}
    stmia r4!,{{r5,r6,r7,r8,r9,r10,r11,r12}}
    ldmia r3!,{{r5,r6,r7,r8,r9,r10,r11,r12}}
    stmia r4!,{{r5,r6,r7,r8,r9,r10,r11,r12}}
    
    // Setup stack pointer for ABT mode.
    cps #{ABORT_MODE} // change to abt mode
    mov sp, #{ABORT_MODE_STACK} // TODO: This should not be a magic number.
    
    // Setup stack pointer for SVC mode.
    cps #{SVC_MODE} // change to svc mode
    ldr sp, =__physical_load_address
    
    // Move the SYSTEM MODE, where the rest of the program will run.
    cpsie aif, #{SYSTEM_MODE}
    ldr sp, ={SYSTEM_MODE_STACK}

zero_bss:
    ldr r3, =__bss_start
    ldr r4, =__bss_end
    mov r5, #0
1:
    str r5, [r3], #4
    cmp r3, r4
    blo 1b
    
enable_mmu:
    // Mask for the top 18 bits
    ldr r3, =0x3FFF
    mvn r0, r3
    // Translation table base address
	ldr	r1, ={TRANSLATION_TABLE}
	mov	r4, #0
    // Mask out the bottom 14 bits
	and	r0, r1, r0
    // Set the correct flags corresponding to the type of memory the translation table is stored in.
	orr	r0, r0, #27
    // Set the translation table base address with its flags
    // See section B4.9.3
	mcr	p15, #0, r0, c2, c0, #0
    // Set the value of `N` to 0 in the translation table base control register
    // See section B4.9.3
	mcr	p15, #0, r4, c2, c0, #2
    // Set all domains to manager mode
    // TODO: figure out how domain work
    // See section B4.9.4
	mvn	r0, #0
	mcr	p15, #0, r0, c3, c0, #0
    
    // Invalidate caches and TLB
    // See section B6.6.5
	mcr	p15, #0, r4, c7, c7, #0
	mcr	p15, #0, r4, c8, c7, #0
    
    // Enable the MMU on the control register
	mrc	p15, #0, r0, c1, c0, #0
    // bit 12: enable L1 instruction cache
    // bit 11: enable branch prediction
    // bit 2: enable L1 data cache
    // bit 0: enable MMU
    // See section B3.4.1
	orr	r0, r0, #5
	orr	r0, r0, #6144
	mcr	p15, #0, r0, c1, c0, #0
    // data synchronization barrier
	mcr	p15, #0, r4, c7, c10, #4

    // Call into Rust.
    b {FIRST_STAGE}

hang:
    b hang

abort_with_panic:
    cpsid aif, #{ABORT_MODE}
    // Store some register values to have better debugging information in gdb.
    mrc p15, 0, r0, c5, c0, 0
    mrc p15, 0, r1, c6, c0, 0
    b {PANIC}

isr:
    // Enter the interrupt in SVC mode.
    srsfd #{SVC_MODE}!
    // This is only called from handlers in which the IRQ is disabled, so no need to disable it.
    cpsie af, #{SVC_MODE}
    // Put all registers on the stack.
    stmfd sp!, {{r0-r12}}
    // ... handle the interrupt ...
    bl interrupt_handler
    // Restore all registers.
    ldmfd sp!, {{r0-r12}}
    rfefd sp!
