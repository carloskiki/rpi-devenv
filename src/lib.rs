//! Hardware abstractions over the BCM2835 microchip.

// IMPORTANT: After the last register read of a peripheral inside of a function, a memory barrier
// must be issued.
// That is because the BCM2835 says that two reads to different peripherals can come out of order.
// This is not a concern for the writes, as they target different peripherals.
// See the BCM2835 manual section 1.3 for more details.
#![no_std]
#![warn(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod gpio;
pub mod uart;

extern "C" {
    pub fn mem_barrier();
}

use bitflags::bitflags;
bitflags! {
    pub struct InterruptTable: u64 {
        const AUXILIARY = 1 << 29;
        const I2C_SPI_PERIPHERAL = 1 << 43;
        const SMI = 1 << 48;
        const GPIO_0 = 1 << 49;
        const GPIO_1 = 1 << 50;
        const GPIO_2 = 1 << 51;
        const GPIO_3 = 1 << 52;
        const I2C = 1 << 53;
        const SPI = 1 << 54;
        const PCM = 1 << 55;
        const UART = 1 << 57;


        const _ = !0;
    }
}

trait Sealed {}

impl Sealed for () {}

macro_rules! impl_sealed {
    ($($t:ty),*) => {
        $(
            impl Sealed for $t {}
        )*
    };
}
pub(crate) use impl_sealed;
