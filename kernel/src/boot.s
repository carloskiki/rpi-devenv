.section ".text.boot"

.globl _start
_start:
    ldr pc, reset_handler
    ldr pc, undefined_handler
    ldr pc, swi_handler
    ldr pc, prefetch_handler
    ldr pc, data_handler
    ldr pc, unused_handler
    ldr pc, irq_handler
    ldr pc, fiq_handler
reset_handler:      .word reset
undefined_handler:  .word hang
swi_handler:        .word hang
prefetch_handler:   .word hang
data_handler:       .word hang
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
    mov sp, #0x8000000

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

hang:
    b hang
