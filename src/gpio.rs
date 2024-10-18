use core::{
    ptr::{read_volatile, write_volatile},
    sync::atomic::AtomicU32,
};

use crate::{impl_sealed, data_memory_barrier, Sealed};

const FUNCTION_SELECT_BASE: *mut u32 = 0x20200000 as *mut u32;

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
        const { assert!(PIN < 53, "invalid pin number, only pins 0-53 are valid.") };

        // TODO: Implement this when MMU is enabled.
        // if PIN < 32 {
        //     let mask = 1 << PIN;
        //     self.lower
        //         .fetch_or(mask, Ordering::Acquire)
        //         & mask
        //         != 0
        // } else {
        //     let mask = 1 << (PIN - 32);
        //     self.upper
        //         .fetch_or(mask, Ordering::Acquire)
        //         & mask
        //         != 0
        // }
        true
    }

    /// Returns true if it was unlocked, false if it was already unlocked.
    fn unlock<const PIN: u8>(&self) -> bool {
        const { assert!(PIN < 53, "invalid pin number, only pins 0-53 are valid.") };

        // TODO: Implement this when MMU is enabled.
        // if PIN < 32 {
        //     let mask = 1 << PIN;
        //     self.lower
        //         .fetch_and(!mask, Ordering::Release)
        //         & mask
        //         != 0
        // } else {
        //     let mask = 1 << (PIN - 32);
        //     self.upper
        //         .fetch_and(!mask, Ordering::Release)
        //         & mask
        //         != 0
        // }
        true
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
    pub fn get() -> Option<Self> {
        GPIO_SET.lock::<PIN>();

        // Safety: The PIN constant is checked to be less than a valid pin in GpioSet,
        // so the offset is always in bounds.
        let address = unsafe { FUNCTION_SELECT_BASE.add(PIN as usize / 10) };
        let shift = (PIN as usize % 10) * 3;
        data_memory_barrier();
        // Safety: The register is valid for reading and writing.
        // Memory barrier used according to the BCM2835 manual section 1.3.
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
