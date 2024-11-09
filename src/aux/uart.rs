//! The Mini UART peripheral, also referred to as `UART1`.

pub mod reader;
mod registers;
pub mod writer;

use core::ptr::{read_volatile, write_volatile};

use crate::{
    aux::uart::registers::*,
    data_memory_barrier,
};

use critical_section::CriticalSection;
use reader::RxPin;
use writer::TxPin;

// This is the clock speed of the "system clock," which is the VPU clock (Video Core).
// TODO: This should be configurable at build time.
pub const CLOCK_SPEED: u32 = 250_000_000;


pub fn pair<RP: RxPin, TP: TxPin>(
    rx_pin: RP,
    tx_pin: TP,
    config: &Config,
) -> (reader::Reader<RP>, writer::Writer<TP>) {
    todo!()
}

/// Safety: Must be called before main.
pub(super) unsafe fn setup(_cs: &CriticalSection) {
    // Disable RX and TX.
    data_memory_barrier();
    // Safety: Addresses valid, data memory barrier used.
    unsafe {
        write_volatile(EXTRA_CONTROL_REG, 0);
    }
    
}

// Handle interrupts that pertain to the Mini UART peripheral.
pub(super) fn interrupt_handler() {
    data_memory_barrier();
    // Safety: Address is valid, data memory barrier used.
    let interrupt_id = unsafe { read_volatile(INTERRUPT_ID_REG) };
    let interrupt_mask = (interrupt_id >> 1) & 0b11;
    if interrupt_mask & 0b1 != 0 {
        writer::interrupt_handler();
    }
    if interrupt_mask & 0b10 != 0 {
        reader::interrupt_handler();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum BitMode {
    #[default]
    SevenBits = 0,
    EightBits = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BaudRate(u32);

impl BaudRate {
    pub fn new(baud_rate: u32) -> Self {
        // TODO: This should depend on the clock speed.
        assert!(
            (476..=31_250_000).contains(&baud_rate),
            "baud rate not in the range 476..=31_250_000"
        );
        Self(baud_rate)
    }

    fn register_value(&self) -> u32 {
        CLOCK_SPEED / (8 * self.0) - 1
    }
}

pub struct Config {
    /// Panics if the baud rate is not in the range `476..=31_250_000`.
    pub baud_rate: BaudRate,
    pub bit_mode: BitMode,
    // TODO: Control flow
}

impl Config {
    fn setup(&self) {
        data_memory_barrier();
        // Safety: Valid address used, data memory barrier used.
        unsafe {
            write_volatile(LINE_CONTROL_REG, self.bit_mode as u32);
            write_volatile(BAUDRATE_REG, self.baud_rate.register_value());
        }
    }
}
