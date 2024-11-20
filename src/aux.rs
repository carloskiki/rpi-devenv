use core::ptr::{read_volatile, write_volatile};

use critical_section::CriticalSection;

use crate::data_memory_barrier;

pub mod spi;
pub mod uart;

/// Auxiliary Interrupt status
/// BCM2835 ARM Peripherals, page 9
pub const AUX_INTERRUPT_STATUS: *mut u32 = 0x20215000 as _;
/// Auxiliary enables
/// BCM2835 ARM Peripherals, page 9
pub const AUX_ENABLES: *mut u32 = 0x20215004 as _;

/// Safety: Must be called before main.
pub(crate) unsafe fn setup(cs: &CriticalSection) {
    data_memory_barrier();
    // Safety: Address valid, data memory barrier used, we have a cs lock.
    unsafe {
        let enable_state = read_volatile(AUX_ENABLES);
        write_volatile(AUX_ENABLES, enable_state | 1);
    }
    // Safety: call before main ensured by the caller.
    unsafe { uart::setup(cs) };
}

pub(crate) fn interrupt_handler() {
    data_memory_barrier();
    // Safety: Address is valid, data memory barrier used.
    let aux_irq = unsafe { read_volatile(AUX_INTERRUPT_STATUS) };
    if aux_irq & 0b1 != 0 {
        uart::interrupt_handler();
    }
    if aux_irq & 0b10 != 0 {
        spi::interrupt_handler();
    }
}
