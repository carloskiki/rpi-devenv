// # Internals
//
// After some testing, here is _probably_ what is happening:
// - All IO/TXHOLD/PEEK point to the same FIFOs (at least same tx for sure).
// - When you write to the TX FIFO, you can only write 4 entries that can be up to 32 bits long (or
//  24 in variable mode).
// - The IO/TXHOLD/PEEK registers are 32 bits wide, forget about the "16 bits" that is said in the
//  documentation.
//
// ### STATUS Register
// - Bits 28-30: TX FIFO level (in bytes)
// - Bits 20-22: RX FIFO level (in bytes)
//
// # Design Decisions
// - We only support variable mode for CS, because supporting fixed mode with arbitrary byte counts
//  would be a pain (we tried and does not fit well at all with the `hal` model),
//  and Linux only supports variable mode as well.
// - We could have an implementation where you can share handles and have a lock free algorithm,
//  but this impl would only be for the blocking API, since interrupts cannot provide wakeups at
//  any FIFO level. So I decided to only have the exclusive access model.

use crate::{hal, Sealed, impl_sealed};

mod registers;

mod implementation;
pub use implementation::Spi1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub speed: Speed,
    pub post_input: bool,
    pub data_out_hold: DataOutHold,
    pub in_rising: bool,
    pub out_rising: bool,
    pub out_most_significant_first: bool,
    pub in_most_significant_first: bool,
    pub extra_cs_high_time: CsHighTime,
    pub keep_input: bool,
    pub polarity: hal::spi::Polarity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Speed(u32);

impl Speed {
    pub fn new(value: u32) -> Self {
        // TODO: Make sure value is in bounds
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataOutHold {
    H0 = 0,
    H1 = 1,
    H4 = 2,
    H7 = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CsHighTime(u8);

impl CsHighTime {
    pub fn new(value: u8) -> Self {
        // TODO: Make sure value is in bounds
        Self(value)
    }
}

trait Spi1CsPin: Sealed {}
