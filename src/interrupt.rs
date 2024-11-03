use core::{
    arch::asm,
    sync::atomic::{compiler_fence, Ordering},
};

use crate::data_memory_barrier;

const INTERRUPT_BASE: usize = 0x2000_B200;
const IRQ_BASIC_PENDING: *mut u32 = INTERRUPT_BASE as *mut u32;
const IRQ_PENDING_1: *mut u32 = (INTERRUPT_BASE + 0x04) as *mut u32;
const IRQ_PENDING_2: *mut u32 = (INTERRUPT_BASE + 0x08) as *mut u32;
const ENABLE_IRQS_1: *mut u32 = (INTERRUPT_BASE + 0x10) as *mut u32;
const ENABLE_IRQS_2: *mut u32 = (INTERRUPT_BASE + 0x14) as *mut u32;
// const ENABLE_BASIC_IRQS: *mut u32 = (INTERRUPT_BASE + 0x18) as *mut u32;

/// Enable interrupts.
///
/// # Safety
///
/// Must not be called inside of a critical section.
pub unsafe fn disable() {
    // Safety: The instruction is defined in the ARMv6 manual. See section A4.1.16.
    unsafe {
        asm!("cpsid i", options(nomem, nostack));
    }
    compiler_fence(Ordering::SeqCst);
}

/// Disable interrupts.
///
/// # Safety
///
/// Must not be called inside of a critical section.
pub unsafe fn enable() {
    // Safety: The instruction is defined in the ARMv6 manual. See section A4.1.16.
    unsafe {
        asm!("cpsie i", options(nomem, nostack));
    }
    compiler_fence(Ordering::SeqCst);
}

/// Set up interrupt sources in the BCM2835.
// TODO: This should not be public when the boot file is added to the lib.
pub fn setup() {
    data_memory_barrier();
    
    let mut enable_irqs_1 = 0;
    let mut enable_irqs_2 = 0;
    for InterruptDescriptor { bit, .. } in INTERRUPT_SOURCES1 {
        enable_irqs_1 |= 1 << bit;
    }
    for InterruptDescriptor { bit, .. } in INTERRUPT_SOURCES2 {
        enable_irqs_2 |= 1 << (bit - 32);
    }
    // Safety: The register is defined in the BCM2835 manual. See section 7.5.
    // A data memory barrier is used to ensure that the writes to the registers are not reordered.
    unsafe {
        ENABLE_IRQS_1.write_volatile(enable_irqs_1);
        ENABLE_IRQS_2.write_volatile(enable_irqs_2);
    }
}

pub struct InterruptDescriptor {
    // TODO: Make sure that this is less than 64, and support basic interrupts.
    pub bit: u8,
    /// # Safety
    ///
    /// This function is allowed to be unsafe, but it must be safe to call from the interrupt
    /// handler.
    pub handler: unsafe fn(),
}

pub(crate) static INTERRUPT_SOURCES1: &[InterruptDescriptor] = &[
    // System Timer Interrupts
    InterruptDescriptor {
        bit: 1,
        handler: crate::system_time::driver::handler_c1,
    },
    InterruptDescriptor {
        bit: 3,
        handler: crate::system_time::driver::handler_c3,
    }
];

pub(crate) static INTERRUPT_SOURCES2: &[InterruptDescriptor] = &[
    // System Timer Interrupts
    InterruptDescriptor {
        bit: 49,
        handler: crate::gpio::interrupt_handler1,
    },
    InterruptDescriptor {
        bit: 50,
        handler: crate::gpio::interrupt_handler2,
    }
];

/// # Safety
///
/// - This must be called when interrupts are disabled.
#[unsafe(no_mangle)]
unsafe extern "C" fn interrupt_handler() {
    // When writing this funciton:
    // - _You only have 16kb of stack space_, don't overflow it.
    // - _Be quick_, because you are _interrupting_ the current execution.
    // - You cannot deterministically cause an interrupt, because then the handler will be called again
    //  in a recursive loop, right after the handler finishes handling the current one.
    
    // The strategy:
    // - Disable interrupts (already done before entering this function).
    // - Read the interrupt register, and handle all pending interrupts.
    // - Clear the interrupts that were handled, so all interrupts that occured before we entered
    //  the isr are cleared.
    // - Enable interrupts, and if an interrupt occured as we were handling the interrupts, the isr
    //  will be called again.
    //
    //  Notes:
    //  - Since we are the isr, we must also have a data memory barrier _after_ handling the
    //  interrupts, because we might have interrupted something in the middle of a read or write
    //  to a peripheral.

    data_memory_barrier();
    // Safety: The register is defined in the BCM2835 manual. See section 7.5.
    // A data memory barrier is used to ensure that the reads from the registers are not
    // reordered.
    let basic_pending = unsafe { IRQ_BASIC_PENDING.read_volatile() };
    // If IRQ_PENDING_1 has some pending interrupts, handle them.
    if basic_pending & (1 << 8) != 0 {
        // Safety: Same as above.
        let pending_irqs = unsafe { IRQ_PENDING_1.read_volatile() };
        for InterruptDescriptor { bit, handler } in INTERRUPT_SOURCES1 {
            if pending_irqs & (1 << bit) != 0 {
                // Safety: We are the interrupt handler.
                unsafe { handler() };
            }
        }
    }
    if basic_pending & (1 << 9) != 0 {
        // Safety: Same as above.
        let pending_irqs = unsafe { IRQ_PENDING_2.read_volatile() };
        for InterruptDescriptor { bit, handler } in INTERRUPT_SOURCES2 {
            if pending_irqs & (1 << (bit - 32)) != 0 {
                // Safety: We are the interrupt handler.
                unsafe { handler() };
            }
        }
    }
    
    // Safety: The register is defined in the BCM2835 manual. See section 7.5.
    // A data memory barrier is used to ensure that the reads from the registers are not
    // reordered.

    data_memory_barrier();
}
