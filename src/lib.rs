//! Hardware abstractions over the BCM2835 microchip.

// IMPORTANT: After the last register read of a peripheral inside of a function, a memory barrier
// must be issued.
// That is because the BCM2835 says that two reads to different peripherals can come out of order.
// This is not a concern for the writes, as they target different peripherals.
// See the BCM2835 manual section 1.3 for more details.
#![no_std]
#![warn(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod aux;
mod critical_section_impl;
pub mod executor;
pub mod gpio;
pub mod interrupt;
pub mod mmu;
pub mod system_time;

use core::{
    arch::{asm, global_asm},
    cell::Cell,
    task::Waker,
};

use ::critical_section::{CriticalSection, Mutex};

pub use embedded_hal as hal;
pub use embedded_hal_async as hal_async;
pub use embedded_hal_nb as hal_nb;
pub use embedded_io as eio;
pub use embedded_io_async as eio_async;
pub use macros::main;

const ABORT_MODE: u32 = 0b10111;
const ABORT_MODE_STACK: u32 = 0x4000;
const SUPERVISOR_MODE: u32 = 0b10011;
const SYSTEM_MODE: u32 = 0b11111;

global_asm!(
    include_str!("boot.s"),
    TRANSLATION_TABLE = sym mmu::TRANSLATION_TABLE,
    ABORT_MODE = const ABORT_MODE,
    ABORT_MODE_STACK = const ABORT_MODE_STACK,
    SVC_MODE = const SUPERVISOR_MODE,
    SYSTEM_MODE = const SYSTEM_MODE,
    SYSTEM_MODE_STACK = const mmu::STACK_TOP,
    PANIC = sym panic,
    FIRST_STAGE = sym first_stage,
);
fn panic() -> ! {
    panic!()
}

#[unsafe(no_mangle)]
extern "C" fn first_stage() -> ! {
    // Initialize peripherals
    ::critical_section::with(|cs| {
        // Safety: The function is called in the first stage of the boot process.
        unsafe { aux::setup(&cs) };
    });

    // Enable interrupts
    interrupt::setup();

    extern "C" {
        #[link_name = "_main"]
        fn main() -> !;
    }
    // Safety: The main function in defined by the user using the `main!` macro.
    unsafe { main() };
}

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
        asm!("mcr p15, 0, {}, c7, c10, 5", in(reg) 0, options(nostack, nomem, preserves_flags));
    }
}

pub fn data_synchronization_barrier() {
    // Safety: The operation is defined in the ARMv6 manual. See section B2.6.2 of the ARMv6 manual,
    // and section 3.2.22 of the ARM1176JZFS manual.
    unsafe {
        asm!("mcr p15, 0, {}, c7, c10, 4", in(reg) 0, options(nostack, nomem, preserves_flags));
    }
}

trait Sealed {}

macro_rules! impl_sealed {
    ($($t:ty),*) => {
        $(
            impl Sealed for $t {}
        )*
    };
    ($($bounds:tt => $type:tt),*) => {
        $(
            impl<$bounds> Sealed for $type {}
        )*
    }
}
use impl_sealed;

type WakerCell = Mutex<Cell<Option<Waker>>>;
#[allow(clippy::declare_interior_mutable_const)]
const WAKER_CELL_INIT: WakerCell = Mutex::new(Cell::new(None));

/// Set the new waker if it is not equal to the old one.
///
/// Returns `true` if a waker was already in the slot, `false` otherwise.
fn set_waker(slot: &WakerCell, waker_ref: &Waker, cs: CriticalSection) -> bool {
    let waker = slot.borrow(cs).take();
    let was_set = waker.is_some();
    let to_set = waker
        .filter(|w| w.will_wake(waker_ref))
        .unwrap_or_else(|| waker_ref.clone());
    slot.borrow(cs).set(Some(to_set));
    was_set
}

fn wake(slot: &WakerCell, cs: CriticalSection) {
    if let Some(waker) = slot.borrow(cs).take() {
        waker.wake();
    }
}
