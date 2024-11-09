use crate::data_memory_barrier;

pub mod uart;

const AUX_IRQ: *const u32 = 0x2021_5000 as _;

pub(crate) fn interrupt_handler() {
    data_memory_barrier();
    // Safety: Address is valid, data memory barrier used.
    let aux_irq = unsafe { core::ptr::read_volatile(AUX_IRQ) };
    if aux_irq & 0b1 != 0 {
        uart::interrupt_handler();
    }
}
