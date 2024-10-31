use core::{
    ptr::{read_volatile, write_volatile},
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{data_memory_barrier, impl_sealed, Sealed};

const FUNCTION_SELECT_BASE: *mut u32 = 0x20200000 as *mut u32;
const GPIO_SET_BASE: *mut u32 = 0x2020001C as *mut u32;
const GPIO_CLEAR_BASE: *mut u32 = 0x20200028 as *mut u32;
const GPIO_LEVEL_BASE: *mut u32 = 0x20200034 as *mut u32;
const GPIO_PULL_CONTROL: *mut u32 = 0x20200094 as *mut u32;
const GPIO_PULL_SET_BASE: *mut u32 = 0x20200098 as *mut u32;

static GPIO_SET: GpioSet = GpioSet::new();

struct GpioSet {
    lower: AtomicU32,
    upper: AtomicU32,
}

impl GpioSet {
    pub const fn new() -> Self {
        Self {
            lower: AtomicU32::new(0),
            upper: AtomicU32::new(0),
        }
    }

    /// Returns true if it was locked, false if it was already locked.
    fn lock<const PIN: u8>(&self) -> bool {
        if PIN < 32 {
            let mask = 1 << PIN;
            self.lower.fetch_or(mask, Ordering::AcqRel) & mask != 0
        } else {
            let mask = 1 << (PIN - 32);
            self.upper.fetch_or(mask, Ordering::AcqRel) & mask != 0
        }
    }

    /// Returns true if it was unlocked, false if it was already unlocked.
    fn unlock<const PIN: u8>(&self) -> bool {
        if PIN < 32 {
            let mask = 1 << PIN;
            self.lower.fetch_and(!mask, Ordering::AcqRel) & mask != 0
        } else {
            let mask = 1 << (PIN - 32);
            self.upper.fetch_and(!mask, Ordering::AcqRel) & mask != 0
        }
    }
}

pub struct Input;
pub struct Output;
pub struct Alternate0;
pub struct Alternate1;
pub struct Alternate2;
pub struct Alternate3;
pub struct Alternate4;
pub struct Alternate5;

pub struct Pin<const PIN: u8, T> {
    _pin: core::marker::PhantomData<T>,
}

impl<const PIN: u8, T: PinType> Pin<PIN, T> {
    /// Get a new pin instance.
    ///
    /// Return `Some(Pin)` if the pin is successfully locked, `None` if the pin is already
    /// used. If the pin number is bigger than 53, this method will panic.
    pub fn get() -> Option<Self> {
        const { assert!(PIN < 53, "invalid pin number, only pins 0-53 are valid.") };
        GPIO_SET.lock::<PIN>();

        // Safety: The PIN constant is checked to be less than a valid pin in GpioSet,
        // so the offset is always in bounds.
        let address = unsafe { FUNCTION_SELECT_BASE.add(PIN as usize / 10) };
        let shift = (PIN as usize % 10) * 3;
        data_memory_barrier();
        // Safety: The register is valid for reading and writing.
        // Memory barrier used.
        let func_sel = unsafe { read_volatile(address) };
        // Safety: The register is valid for writing.
        unsafe {
            write_volatile(
                address,
                (func_sel & !(0b111 << shift)) | (T::MODE_BITS << shift),
            )
        };

        Some(Pin {
            _pin: core::marker::PhantomData,
        })
    }
}

impl<const PIN: u8> Pin<PIN, Output> {
    /// Set the pin high.
    pub fn set(&self) {
        data_memory_barrier();
        // Safety: Both this address and the following one are valid for writing.
        // Memory barrier used.
        unsafe {
            double_register_op::<PIN, _, _>(GPIO_SET_BASE, |addr, mask| {
                write_volatile(addr, mask);
            })
        }
    }

    /// Set the pin low.
    pub fn clear(&self) {
        data_memory_barrier();
        // Safety: Both this address and the following one are valid for writing.
        // Memory barrier used.
        unsafe {
            double_register_op::<PIN, _, _>(GPIO_CLEAR_BASE, |addr, mask| {
                write_volatile(addr, mask);
            })
        }
    }

    /// Returns `true` if the pin is high, `false` if the pin is low.
    pub fn level(&self) -> bool {
        data_memory_barrier();
        // Safety: Both this address and the following one are valid for reading.
        // Memory barrier used.
        unsafe {
            double_register_op::<PIN, _, _>(GPIO_LEVEL_BASE, |addr, mask| {
                let level = read_volatile(addr);
                level & mask != 0
            })
        }
    }
}

impl<const PIN: u8> Pin<PIN, Input> {
    pub fn level(&self) -> bool {
        data_memory_barrier();
        // Safety: Both this address and the following one are valid for reading.
        // Memory barrier used.
        unsafe {
            double_register_op::<PIN, _, _>(GPIO_LEVEL_BASE, |addr, mask| {
                let level = read_volatile(addr);
                level & mask != 0
            })
        }
    }

    pub fn set_pull(&self, pull: Option<PullState>) {
        let pull = match pull {
            Some(PullState::Up) => 0b10,
            Some(PullState::Down) => 0b01,
            None => 0b00,
        };

        data_memory_barrier();
        // Safety: The address is valid for writing.
        // Memory barrier used.
        unsafe {
            write_volatile(GPIO_PULL_CONTROL, pull);
        }

        // Wait 150 clock cycles according to manual p. 101.
        for _ in 0..150 {
            core::hint::spin_loop();
        }

        // Safety: Both this address and the following one are valid for writing.
        // Memory barrier used.
        unsafe {
            double_register_op::<PIN, _, _>(GPIO_PULL_SET_BASE, |addr, mask| {
                write_volatile(addr, mask);
            })
        }

        // Wait another 150 clock cycles according to manual p. 101.
        for _ in 0..150 {
            core::hint::spin_loop();
        }

        // Safety: Both this address and the following one are valid for writing.
        // Memory barrier used.
        unsafe { write_volatile(GPIO_PULL_CONTROL, 0) };
    }
}

/// The pull state of a pin.
pub enum PullState {
    /// Pin is pulled up.
    Up,
    /// Pin is pulled down.
    Down,
}

/// # Safety
///
/// The caller must ensure that the adderss following the base is valid for the provided operation.
unsafe fn double_register_op<const PIN: u8, F, O>(base: *mut u32, f: F) -> O
where
    F: FnOnce(*mut u32, u32) -> O,
{
    let offset = PIN / 32;
    let mask = 1 << (PIN % 32);
    // Safety: The offset is always 0 or 1, and other requirements are met by the caller.
    unsafe { f(base.add(offset as usize), mask) }
}

impl<const PIN: u8, T> Drop for Pin<PIN, T> {
    fn drop(&mut self) {
        GPIO_SET.unlock::<PIN>();
    }
}

impl<const PIN: u8, T> Sealed for Pin<PIN, T> {}

#[allow(private_bounds)]
pub trait PinType: Sealed {
    const MODE_BITS: u32;
}

impl_sealed!(Input, Output, Alternate0, Alternate1, Alternate2, Alternate3, Alternate4, Alternate5);

impl PinType for Input {
    const MODE_BITS: u32 = 0b000;
}
impl PinType for Output {
    const MODE_BITS: u32 = 0b001;
}
impl PinType for Alternate0 {
    const MODE_BITS: u32 = 0b100;
}
impl PinType for Alternate1 {
    const MODE_BITS: u32 = 0b101;
}
impl PinType for Alternate2 {
    const MODE_BITS: u32 = 0b110;
}
impl PinType for Alternate3 {
    const MODE_BITS: u32 = 0b111;
}
impl PinType for Alternate4 {
    const MODE_BITS: u32 = 0b011;
}
impl PinType for Alternate5 {
    const MODE_BITS: u32 = 0b010;
}
