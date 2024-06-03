use core::{ptr::{read_volatile, write_volatile}, sync::atomic::{AtomicU32, Ordering}};

use crate::{delay, mem_barrier};

const FUNCTION_SELECT_BASE: *mut u32 = 0x20200000 as *mut u32;

static GPIO_USED_SET: AtomicU32 = AtomicU32::new(0);

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

#[allow(private_bounds)]
impl<const PIN: u8, T: sealed::PinType> Pin<PIN, T> {
    pub fn get() -> Option<Self> {
        if PIN > 27 {
            return None;
        }
        
        let mask = 1 << PIN;
        let used_set = GPIO_USED_SET.load(Ordering::Acquire);
        if used_set & mask != 0 {
            return None;
        }
        GPIO_USED_SET.store(used_set | mask, Ordering::Release);
        
        // Safety: The PIN constant is checked to be less than 28, so the offset is always in bounds.
        let address = unsafe { FUNCTION_SELECT_BASE.add(PIN as usize / 10) };
        let shift = (PIN as usize % 10) * 3;
        // Safety: The register is valid for reading and writing.
        let func_sel = unsafe { read_volatile(address) };
        // Safety: Memory barrier used according to the BCM2835 manual section 1.3.
        unsafe { mem_barrier() };
        // Safety: The register is valid for writing.
        unsafe { write_volatile(address, (func_sel & !(0b111 << shift)) | (T::MODE_BITS << shift)) };
        
        Some(Pin {
            _pin: core::marker::PhantomData,
        })
    }
}

impl<const PIN: u8, T> Drop for Pin<PIN, T> {
    fn drop(&mut self) {
        let mask = 1 << PIN;
        let used_set = GPIO_USED_SET.load(Ordering::Acquire);
        GPIO_USED_SET.store(used_set & !mask, Ordering::Release);
    }
}

mod sealed {
    pub(super) trait PinType {
        const MODE_BITS: u32;
    }

    impl PinType for super::Input {
        const MODE_BITS: u32 = 0b000;
    }
    impl PinType for super::Output {
        const MODE_BITS: u32 = 0b001;
    }
    impl PinType for super::Alternate0 {
        const MODE_BITS: u32 = 0b100;
    }
    impl PinType for super::Alternate1 {
        const MODE_BITS: u32 = 0b101;
    }
    impl PinType for super::Alternate2 {
        const MODE_BITS: u32 = 0b110;
    }
    impl PinType for super::Alternate3 {
        const MODE_BITS: u32 = 0b111;
    }
    impl PinType for super::Alternate4 {
        const MODE_BITS: u32 = 0b011;
    }
    impl PinType for super::Alternate5 {
        const MODE_BITS: u32 = 0b010;
    }
}
