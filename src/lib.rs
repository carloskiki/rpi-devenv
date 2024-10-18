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
mod mmu;

use core::arch::asm;

// use bitflags::bitflags;
// bitflags! {
//     pub struct InterruptTable: u64 {
//         const AUXILIARY = 1 << 29;
//         const I2C_SPI_PERIPHERAL = 1 << 43;
//         const SMI = 1 << 48;
//         const GPIO_0 = 1 << 49;
//         const GPIO_1 = 1 << 50;
//         const GPIO_2 = 1 << 51;
//         const GPIO_3 = 1 << 52;
//         const I2C = 1 << 53;
//         const SPI = 1 << 54;
//         const PCM = 1 << 55;
//         const UART = 1 << 57;
// 
// 
//         const _ = !0;
//     }
// }

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

/// Perform a data memory barrier operation.
/// 
/// All explicit memory accesses occurring in program order before this operation
/// will be globally observed before any memory accesses occurring in program
/// order after this operation. This includes both read and write accesses.
/// 
/// This differs from a "data synchronization barrier" in that a data
/// synchronization barrier will ensure that all previous explicit memory
/// accesses occurring in program order have fully completed before continuing
/// and that no subsequent instructions will be executed until that point, even
/// if they do not access memory. This is unnecessary for what we need this for.
///
/// See section B2.6.1 of the ARMv6 manual for more details.
pub fn data_memory_barrier() {
    // Safety: The operation is defined in the ARMv6 manual. See section B2.6.1 of the ARMv6 manual,
    // and section 3.2.22 of the ARM1176JZFS manual.
    unsafe {
        asm!("mcr p15, 0, {}, c7, c10, 5", in(reg) 0, options(nostack));
    }
}
