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
    mov sp, #0x4000 // TODO: This should not be a magic number.
    
    // Setup stack pointer for SVC mode.
    cps #{SVC_MODE} // change to svc mode
    ldr sp, =__physical_load_address

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

hang:
    b hang

abort_with_panic:
    cpsid aif, #{ABORT_MODE}
    // TODO: Store some register values to have better debugging information.
    b interrupt_panic

isr:
    // Enter the instruction in SVC mode.
    srsfd #{SVC_MODE}!
    cpsie aif, #{SVC_MODE}
    // Put all registers on the stack.
    stmfd sp!, {{r0-r12}}
    // ... handle the interrupt ...
    bl interrupt_handler
    // Restore all registers.
    ldmfd sp!, {{r0-r12}}
    rfefd sp!

to_system_mode:
    cpsie aif, #{SYSTEM_MODE}
    ldr sp, ={SYSTEM_MODE_STACK}
    bx lr
