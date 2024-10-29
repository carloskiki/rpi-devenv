use core::{
    arch::asm,
    marker::PhantomData,
    ptr::null_mut,
    sync::atomic::{AtomicBool, Ordering},
};

use embassy_executor::Spawner;

static WORK_SIGNALED: AtomicBool = AtomicBool::new(false);

#[unsafe(export_name = "__pender")]
fn __pender(_context: *mut ()) {
    WORK_SIGNALED.store(true, Ordering::SeqCst);
}

pub struct Executor {
    raw: embassy_executor::raw::Executor,
    not_send: PhantomData<*mut ()>,
}

impl Executor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run(&'static mut self, init: impl FnOnce(Spawner)) -> ! {
        init(self.raw.spawner());

        loop {
            // Safety: We are not in the pender function, so this is safe.
            unsafe { self.raw.poll() };

            // Critical section to prevent an interrupt occuring after the store but before the
            // WFI
            critical_section::with(|_| {
                // Why AcqRel is not enough:
                // The load here synchronizes with the store from the `__pender`, or ourselves, but the store
                // from the `__pender` does not synchronize with anything. So what could
                // happen is that we synchronize with a store from ourselves and see `false`, while
                // a store from `__pender` happened but is still "in flight." This can happen
                // because `AcqRel` only imposes ordering on the modification order of the atomic
                // with Release-Acquire synchronization, which is insufficient in our case.
                //
                // With `SeqCst` if we see `false` from the load, then we are
                // certain that the previous store was done by us. So the modification order
                // guarantees from the `SeqCst` ensures that the store from `__pender` is seen by
                // us, or happens during `WFI`, in which case it will be seen.
                if !WORK_SIGNALED.swap(false, Ordering::SeqCst) {
                    // Wait for Interrupt
                    // Safety: The instruction is defined in the ARMv6 manual. See section
                    // B6.6.5.
                    unsafe {
                        asm!("mcr p15, 0, {}, c7, c0, 4", out(reg) _, options(nostack, nomem, preserves_flags))
                    };
                }
            });
            // Once woken from WFI, interrupts occur here.
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self {
            raw: embassy_executor::raw::Executor::new(null_mut()),
            not_send: PhantomData,
        }
    }
}
