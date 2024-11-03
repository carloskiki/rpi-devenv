use bitflags::bitflags;

use crate::{impl_sealed, Sealed};

use super::Pin;

const RISING_EDGE_DETECT_BASE: *mut u32 = 0x2020004C as *mut u32;
const FALLING_EDGE_DETECT_BASE: *mut u32 = 0x20200058 as *mut u32;
const HIGH_DETECT_BASE: *mut u32 = 0x20200064 as *mut u32;
const LOW_DETECT_BASE: *mut u32 = 0x20200070 as *mut u32;
const ASYNC_RISING_DETECT_BASE: *mut u32 = 0x2020007C as *mut u32;
const ASYNC_FALLING_DETECT_BASE: *mut u32 = 0x20200088 as *mut u32;

/// The pull state of a pin.
pub enum Pull {
    /// Pin is pulled down.
    Down,
    /// Pin is pulled up.
    Up,
}

impl From<bool> for Pull {
    fn from(value: bool) -> Self {
        match value {
            true => Pull::Up,
            false => Pull::Down,
        }
    }
}

impl From<Pull> for bool {
    fn from(pull: Pull) -> bool {
        match pull {
            Pull::Up => true,
            Pull::Down => false,
        }
    }
}

bitflags! {
    /// The edge to detect on a pin.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct DetectState: u8 {
        /// Detect a level change, low to high.
        ///
        /// This method uses the system clock to sample the pin, in order to reduce the chance of
        /// glitches causing false positives. See [`DetectState::ASYNC_RISING_EDGE`] for an asynchronous
        /// alternative that triggers even for very short pulses.
        const RISING_EDGE = 1;
        /// Detect a level change, low to high.
        ///
        /// This method uses the system clock to sample the pin, in order to reduce the chance of
        /// glitches causing false positives. See [`DetectState::ASYNC_FALLING_EDGE`] for an asynchronous
        /// alternative that triggers even for very short pulses.
        const FALLING_EDGE = 1 << 1;
        /// Detect the pin level being high.
        const HIGH = 1 << 2;
        /// Detect the pin level being low.
        const LOW = 1 << 3;
        /// Detect a low to high change asynchronously.
        ///
        /// This means that the pin will trigger an interrupt as soon as the level changes, the line is
        /// not sampled by the system clock like [`DetectState::RISING_EDGE`]. This also means that glitches
        /// can cause false positives.
        const ASYNC_RISING_EDGE = 1 << 4;
        /// Detect a low to high change asynchronously.
        ///
        /// This means that the pin will trigger an interrupt as soon as the level changes, the line is
        /// not sampled by the system clock like [`DetectState::FALLING_EDGE`]. This also means that glitches
        /// can cause false positives.
        const ASYNC_FALLING_EDGE = 1 << 5;
    }
}

impl DetectState {
    pub(crate) fn registers(&self) -> impl Iterator<Item = *mut u32> {
        self.iter_names().map(|(_, flag)| {
            match flag {
                DetectState::RISING_EDGE => RISING_EDGE_DETECT_BASE,
                DetectState::FALLING_EDGE => FALLING_EDGE_DETECT_BASE,
                DetectState::HIGH => HIGH_DETECT_BASE,
                DetectState::LOW => LOW_DETECT_BASE,
                DetectState::ASYNC_RISING_EDGE => ASYNC_RISING_DETECT_BASE,
                DetectState::ASYNC_FALLING_EDGE => ASYNC_FALLING_DETECT_BASE,
                // The iterator does not yield unknown flags.
                _ => unreachable!(),
            }
        })
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
