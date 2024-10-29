use core::{
    cell::Cell,
    ptr::{read_volatile, write_volatile},
};

use critical_section::Mutex;
use embassy_time_driver::{time_driver_impl, AlarmHandle, Driver};

use crate::data_memory_barrier;

const SYSTEM_TIME_BASE: u64 = 0x2000_3000;
const SYSTEM_TIME_CS: *mut u32 = SYSTEM_TIME_BASE as *mut u32;
const SYSTEM_TIME_CLO: *mut u32 = (SYSTEM_TIME_BASE + 0x04) as *mut u32;
const SYSTEM_TIME_CHI: *mut u32 = (SYSTEM_TIME_BASE + 0x08) as *mut u32;
// C0 and C2 are used by the VC4 firmware.
const SYSTEM_TIME_C1: *mut u32 = (SYSTEM_TIME_BASE + 0x10) as *mut u32;
const SYSTEM_TIME_C3: *mut u32 = (SYSTEM_TIME_BASE + 0x18) as *mut u32;

#[derive(Clone, Copy)]
struct AlarmState {
    timestamp: u64,
    // This is really a Option<(fn(*mut ()), *mut ())>
    // but fn pointers aren't allowed in const yet
    callback: fn(*mut ()),
    ctx: *mut (),
}

// Safety: AlarmState is Send because the ctx pointer should not allow shared mutable access.
unsafe impl Send for AlarmState {}

impl AlarmState {
    const fn null() -> Self {
        AlarmState {
            timestamp: 0,
            callback: |_: *mut ()| {},
            ctx: core::ptr::null_mut(),
        }
    }
}

pub struct SystemTimeDriver {
    // bit 0 for C1, and bit 1 for C3.
    c1: Mutex<Cell<Option<AlarmState>>>,
    c3: Mutex<Cell<Option<AlarmState>>>,
}

impl SystemTimeDriver {
    fn alarm_interrupt(&self, is_c1: bool) {
        data_memory_barrier();
        let slot = if is_c1 {
            // Safety: We are reading from a register that is defined in the BCM2835 manual p. 173.
            // A data barrier is used to ensure that the read is not reordered with another.
            unsafe { write_volatile(SYSTEM_TIME_CS, 1 << 1) };
            &self.c1
        } else {
            // Safety: Same as above.
            unsafe { write_volatile(SYSTEM_TIME_CS, 1 << 3) };
            &self.c3
        };

        critical_section::with(|cs| {
            let AlarmState {
                timestamp,
                callback,
                ctx,
            } = slot.borrow(cs).get().expect("Alarm should be set");
            if timestamp <= self.now() {
                slot.borrow(cs).set(None);
                callback(ctx);
            } else {
                // Safety: We have checked to make sure that timestamp is in the future, so this is safe.
                unsafe { set_alarm(slot.borrow(cs), timestamp) };
            }
        });
    }
}

impl Driver for SystemTimeDriver {
    fn now(&self) -> u64 {
        data_memory_barrier();
        // Safety: We are reading from a register that is defined in the BCM2835 manual p. 173.
        // A data barrier is used to ensure that the read is not reordered with another
        // peripheral.
        let hi = unsafe { read_volatile(SYSTEM_TIME_CHI) };
        // Safety: Same as above.
        let lo = unsafe { read_volatile(SYSTEM_TIME_CLO) };
        // Safety: Same as above.
        let check = unsafe { read_volatile(SYSTEM_TIME_CHI) };
        if check != hi {
            self.now()
        } else {
            ((hi as u64) << 32) | lo as u64
        }
    }

    unsafe fn allocate_alarm(&self) -> Option<AlarmHandle> {
        critical_section::with(|cs| {
            if self.c1.borrow(cs).get().is_none() {
                self.c1.borrow(cs).set(Some(AlarmState::null()));
                // Safety: We are the time driver, so we respect the invariants.
                Some(unsafe { AlarmHandle::new(0) })
            } else if self.c3.borrow(cs).get().is_none() {
                self.c3.borrow(cs).set(Some(AlarmState::null()));
                // Safety: We are the time driver, so we respect the invariants.
                Some(unsafe { AlarmHandle::new(1) })
            } else {
                None
            }
        })
    }

    fn set_alarm_callback(&self, alarm: AlarmHandle, callback: fn(*mut ()), ctx: *mut ()) {
        let state = AlarmState {
            timestamp: 0,
            callback,
            ctx,
        };

        let slot = if alarm.id() == 0 { &self.c1 } else { &self.c3 };

        critical_section::with(|cs| {
            slot.borrow(cs).set(Some(state));
        });
    }

    fn set_alarm(&self, alarm: AlarmHandle, timestamp: u64) -> bool {
        let (slot, mmio_ptr) = if alarm.id() == 0 {
            (&self.c1, SYSTEM_TIME_C1)
        } else {
            (&self.c3, SYSTEM_TIME_C3)
        };

        if timestamp <= self.now() {
            critical_section::with(|cs| {
                slot.borrow(cs).set(None);
            });

            return false;
        }

        critical_section::with(|cs| {
            // Safety: The slot is set to `Some` because we have an alarm handle, and the timestamp
            // was checked to be in the future.
            unsafe { set_alarm(slot.borrow(cs), timestamp) };
        });

        let to_set = core::cmp::min(timestamp.saturating_sub(self.now()), u32::MAX as u64) as u32;
        data_memory_barrier();
        // Safety: We are writing to a register that is defined in the BCM2835 manual p. 173. A
        // data barrier is used as the manual requires.
        unsafe { write_volatile(mmio_ptr, to_set) };

        if timestamp <= self.now() {
            // Here we have a race condition because the interrupt may or may not have been
            // triggered yet. We disable the modified bit in the control/status register so that if
            // it was not, then it will not be triggered, and if it was, then this is a no-op.
            let bitmask = if alarm.id() == 0 { 1 << 1 } else { 1 << 3 };

            critical_section::with(|cs| {
                // Safety: We are writing to a register that is defined in the BCM2835 manual p. 173.
                // A data barrier was used as the manual requires.
                let mut control_register = unsafe { read_volatile(SYSTEM_TIME_CS) };
                control_register &= !bitmask;
                // Safety: We are writing to a register that is defined in the BCM2835 manual p. 173.
                // A data barrier was used as the manual requires.
                unsafe { write_volatile(SYSTEM_TIME_CS, control_register) };

                slot.borrow(cs).set(None);
            });
            return false;
        }

        // We are confident that the interrupt will be triggered.
        true
    }
}

// # Safety
//
// This function should only be called with the slot currently set to `Some`, and the timestamp
// should be checked to be in the future.
unsafe fn set_alarm(slot: &Cell<Option<AlarmState>>, timestamp: u64) {
    let mut state = slot.take().expect("Alarm should be set");
    state.timestamp = timestamp;
    slot.set(Some(state));
}

pub(crate) fn handler_c1() {
    DRIVER.alarm_interrupt(true);
}
pub(crate) fn handler_c3() {
    DRIVER.alarm_interrupt(false);
}

time_driver_impl!(static DRIVER: SystemTimeDriver = SystemTimeDriver {
    c1: Mutex::new(Cell::new(None)),
    c3: Mutex::new(Cell::new(None)),
});

pub static DEBUG_DRIVER: SystemTimeDriver = SystemTimeDriver {
    c1: Mutex::new(Cell::new(None)),
    c3: Mutex::new(Cell::new(None)),
};