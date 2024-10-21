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
undefined_handler:  .word interrupt_panic
swi_handler:        .word hang
prefetch_handler:   .word interrupt_panic
data_handler:       .word interrupt_panic
unused_handler:     .word hang
irq_handler:        .word hang
fiq_handler:        .word hang

reset:
    // Setup the interrupt vector table.
    mov r0,#0x8000
    mov r1,#0x0000
    ldmia r0!,{r2,r3,r4,r5,r6,r7,r8,r9}
    stmia r1!,{r2,r3,r4,r5,r6,r7,r8,r9}
    ldmia r0!,{r2,r3,r4,r5,r6,r7,r8,r9}
    stmia r1!,{r2,r3,r4,r5,r6,r7,r8,r9}

    // Setup the IRQ stack ptr.
    // TODO: This collides with the SVC stack ptr. We should fix this.
    mov r0, #0xD2 // (SPR_MODE_IRQ | SPR_IRQ_DISABLE | SPR_FIQ_DISABLE)
    msr CPSR_c, r0
    mov sp, #0x8000

    // Setup the FIQ stack ptr.
    mov r0, #0xD1 // (SPR_MODE_FIQ | SPR_IRQ_DISABLE | SPR_FIQ_DISABLE)
    msr CPSR_c, r0
    mov sp, #0x4000

    // Setup the SVC stack ptr.
    mov r0, #0xD3 // (SPR_MODE_SVC | SPR_IRQ_DISABLE | SPR_FIQ_DISABLE)
    msr CPSR_c, r0
    mov sp, #0x8000

    // Zero out the BSS section.
zero_bss:
    ldr r3, =__bss_start
    ldr r4, =__bss_end
    mov r5, #0

1:
    str r5, [r3], #4
    cmp r3, r4
    // This break is pc-relative, so it does not use the relocated address.
    // See: https://sourceware.org/binutils/docs/as/Symbol-Names.html.
    blo 1b
    
    // Call into Rust.
    b first_stage

hang:
    b hang
