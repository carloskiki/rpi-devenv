use core::arch::asm;

use critical_section::{set_impl, Impl, RawRestoreState};

use crate::interrupt;

struct SingleCoreCriticalSection;

// Safety: The implementation upholds the safety invariants of the `aquire` and `release`
// functions.
unsafe impl Impl for SingleCoreCriticalSection {
    unsafe fn acquire() -> RawRestoreState {
        let mut cpsr: u32;
        // Safety: The instruction is defined in the ARMv6 manual. See section A4.1.32.
        unsafe { asm!("mrs {}, cpsr", out(reg) cpsr, options(nomem, nostack, preserves_flags)) };
        if cpsr & 1 << 7 == 0 {
            // Safety: We are not inside of a critical section.
            // The syncrhonization is done inside of the `disable` function.
            unsafe { interrupt::disable() };
            true
        } else {
            false
        }
    }

    unsafe fn release(restore_state: RawRestoreState) {
        if restore_state {
            // Safety: We are allowed to enable the interrupts since we are running in system
            // mode.
            // The syncrhonization is done inside of the `enable` function.
            unsafe { interrupt::enable() };
        }
    }
}

set_impl!(SingleCoreCriticalSection);
