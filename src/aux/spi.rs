use core::{convert::Infallible, sync::atomic::AtomicBool};

// # Internals
//
// After some testing, here is _probably_ what is happening:
// - All IO/TXHOLD/PEEK point to the same FIFOs (at least same tx for sure).
// - When you write to the TX FIFO, you can only write 4 entries that can be up to 32 bits long (or
//  24 in variable mode).
// - The FIFOs are 4 entries deep (with entries of any size).
// - The IO/TXHOLD/PEEK registers are 32 bits wide, forget about the "16 bits" that is said in the
//  documentation.
//
// ### STATUS Register
// - Bits 28-30: TX FIFO level (in bytes)
// - Bits 20-22: RX FIFO level (in bytes)
use crate::{
    data_memory_barrier, data_synchronization_barrier,
    gpio::{self, state::Alternate4},
    hal, hal_async, hal_nb,
};

use super::AUX_ENABLES;

pub mod mode;
mod registers;
use embedded_hal::spi::SpiBus;
use registers::{CONTROL0, CONTROL1, IO, SPI1, STATUS, TXHOLD};

// Miscellaneous thoughts:
// - We only support variable mode for CS, because supporting fixed mode with arbitrary byte counts
//  would be a pain (we tried and does not fit well at all with the `hal` model),
//  and Linux only supports variable mode as well.
// - We could have an implementation where you can share handles and have a lock free algorithm,
//  but this impl would only be for the blocking API, since interrupts cannot provide wakeups at
//  any FIFO level. So I decided to only have the exclusive access model.

// Fns to add:
// - Clear fifos
// - Invert Clock

/// Aux SPI peripheral
///
/// # Implmenetation notes
/// - On `read` transactions, we write what is initially present in the buffer.
/// - When writing in `Fixed<N>` mode, if the number of bits per burst does not divide the number
///     of bits provided, padding zeros are added to the last burst. For example, the mode
///     `Fixed<15>` is chosen, and we `write(&[0x12, 0x34, 0x56, 0x78])`, then burst 1 will send
///     15 bits, burst 2 as well, and burst 3 will send the two last bytes, along with 13 zeros.
pub struct Spi1 {
    _miso: gpio::Pin<19, Alternate4>,
    _mosi: gpio::Pin<20, Alternate4>,
    _sclk: gpio::Pin<21, Alternate4>,
}

impl Spi1 {
    pub fn get(
        miso: gpio::Pin<19, Alternate4>,
        mosi: gpio::Pin<20, Alternate4>,
        sclk: gpio::Pin<21, Alternate4>,
        config: &Config,
    ) -> Option<Self> {
        data_memory_barrier();
        critical_section::with(|_| {
            // Safety: Register is valid, data barrier used.
            let aux_enables = unsafe { AUX_ENABLES.read_volatile() };
            if aux_enables & (0b1 << 1) != 0 {
                return None;
            }
            // Safety: Same as above.
            unsafe { AUX_ENABLES.write_volatile(aux_enables | (0b1 << 1)) };
            Some(())
        })?;

        // We have exclusive access to the peripheral, so we can do whatever with the registers.
        let cntl0 = config.speed.0 << 20
            | (config.post_input as u32) << 16
            | 1 << 14 // Variable mode
            | (config.data_out_hold as u32) << 12
            | 1 << 11 // Enable
            | (config.in_rising as u32) << 10
            | (config.out_rising as u32) << 8
            | ((config.polarity == hal::spi::Polarity::IdleHigh) as u32) << 7
            | (config.out_most_significant_first as u32) << 6;
        // Safety: As above.
        unsafe { SPI1.add(CONTROL0).write_volatile(cntl0) };

        let cntl1 = (config.extra_cs_high_time.0 as u32) << 8
            | (config.in_most_significant_first as u32) << 1
            | config.keep_input as u32;
        // Safety: As above.
        unsafe { SPI1.add(CONTROL1).write_volatile(cntl1) };

        Some(Spi1 {
            _miso: miso,
            _mosi: mosi,
            _sclk: sclk,
        })
    }

    /// Is the SPI peripheral busy?
    ///
    /// # Safety
    ///
    /// A data memory barrier must have been used
    unsafe fn busy(&self) -> bool {
        // Safety: Address is valid, data memory barrier ensured by the caller.
        unsafe { SPI1.add(STATUS).read_volatile() >> 6 & 1 == 1 }
    }

    pub fn clear_fifos(&mut self) {
        data_memory_barrier();
        // Safety: Adress valid, data barrier used, and we have exclusive access.
        let reg = unsafe { SPI1.add(CONTROL0).read() };
        // Safety: As above.
        unsafe { SPI1.add(CONTROL0).write(reg | 1 << 9) };
        // we use a data memory barrier because I don't think simply going on and off is enough, we
        // probably have to wait some time.
        data_synchronization_barrier();
        // Safety: As above.
        unsafe { SPI1.add(CONTROL0).write(reg) };
    }
}

impl hal::spi::ErrorType for Spi1 {
    type Error = Infallible;
}

impl hal::spi::SpiBus for Spi1 {
    fn read(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        self.transfer_in_place(words)
    }

    fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        data_memory_barrier();

        // Safety: Address valid, data memory barrier used.
        let ms_bit_first = unsafe { SPI1.add(CONTROL0).read_volatile() } >> 6 & 1 != 0;
        for (index, chunk) in words.chunks(3).enumerate() {
            let mut entry = 0;
            if ms_bit_first {
                for (i, byte) in chunk.iter().enumerate() {
                    entry |= (*byte as u32) << ((chunk.len() - i) * 8);
                }
            } else {
                for (i, byte) in chunk.iter().enumerate() {
                    entry |= (*byte as u32) << (i * 8);
                }
            }
            entry |= (chunk.len() as u32 * 8) << 24; // 24 bits shift len

            // Safety: As above.
            while unsafe { SPI1.add(STATUS).read_volatile() >> 10 & 1 } == 1 {}
            if (index + 1) * 3 >= words.len() {
                // Safety: As above.
                unsafe { SPI1.add(IO).write_volatile(entry) };
            } else {
                // Safety: As above.
                unsafe { SPI1.add(TXHOLD).write_volatile(entry) };
            }
        }

        Ok(())
    }

    fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        if read.len() >= write.len() {
            read[0..write.len()].copy_from_slice(write);
            self.transfer_in_place(read)
        } else {
            read.copy_from_slice(&write[0..read.len()]);
            self.transfer_in_place(read)?;
            self.write(&write[read.len()..])
        }
    }

    fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        data_memory_barrier();
        self.flush()?;
        self.clear_fifos();

        // Safety: Address valid, data memory barrier used.
        let out_ms_bit_first = unsafe { SPI1.add(CONTROL0).read_volatile() } >> 6 & 1 != 0;
        // Safety: As above.
        let in_ms_bit_first = unsafe { SPI1.add(CONTROL1).read_volatile() } & 1 != 0;
        let words_len = words.len();
        for (index, chunk) in words.chunks_mut(3).enumerate() {
            let chunk_len = chunk.len();
            let mut entry = 0;
            if out_ms_bit_first {
                for (i, byte) in chunk.iter().enumerate() {
                    entry |= (*byte as u32) << ((chunk_len - i) * 8);
                }
            } else {
                for (i, byte) in chunk.iter().enumerate() {
                    entry |= (*byte as u32) << (i * 8);
                }
            }
            entry |= (chunk.len() as u32 * 8) << 24;

            if (index + 1) * 3 >= words_len {
                // Safety: As above.
                unsafe { SPI1.add(IO).write_volatile(entry) };
            } else {
                // Safety: As above.
                unsafe { SPI1.add(TXHOLD).write_volatile(entry) };
            }

            // Safety: As above.
            while unsafe { SPI1.add(STATUS).read_volatile() >> 7 & 1 } == 1 {}
            // Safety: As above.
            let entry = unsafe { SPI1.add(IO).read_volatile() };
            if in_ms_bit_first {
                for (i, byte) in chunk.iter_mut().enumerate() {
                    *byte = (entry >> ((chunk_len - i) * 8)) as u8;
                }
            } else {
                for (i, byte) in chunk.iter_mut().enumerate() {
                    *byte = (entry >> (i * 8)) as u8;
                }
            }
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        data_memory_barrier();
        // Safety: data barrier used.
        while unsafe { self.busy() } {
            core::hint::spin_loop();
        }
        Ok(())
    }
}

impl hal_async::spi::SpiBus for Spi1 {
    fn read(
        &mut self,
        words: &mut [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> {
        <Self as hal_async::spi::SpiBus>::transfer_in_place(self, words)
    }

    async fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        todo!()
    }

    async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        if read.len() >= write.len() {
            read[0..write.len()].copy_from_slice(write);
            <Self as hal_async::spi::SpiBus>::transfer_in_place(self, read).await
        } else {
            read.copy_from_slice(&write[0..read.len()]);
            <Self as hal_async::spi::SpiBus>::transfer_in_place(self, read).await?;
            <Self as hal_async::spi::SpiBus>::write(self, &write[read.len()..]).await
        }
    }

    async fn transfer_in_place(
        &mut self,
        words: &mut [u8],
    ) -> Result<(), Self::Error> {
        todo!()
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        todo!()
    }
}

/// One cannot use both this API and the `SpiBus` API at the same time. If needed, one should call
/// `clear_fifos` between APIs switches.
impl hal_nb::spi::FullDuplex for Spi1 {
    fn read(&mut self) -> hal_nb::nb::Result<u8, Self::Error> {
        data_memory_barrier();

        // Safety: Address valid, data barrier used.
        if unsafe { SPI1.add(STATUS).read_volatile() >> 7 & 1 == 1 } {
            return Err(hal_nb::nb::Error::WouldBlock);
        }

        // Safety: As above.
        Ok(unsafe { SPI1.add(IO).read_volatile() as u8 })
    }

    fn write(&mut self, word: u8) -> hal_nb::nb::Result<(), Self::Error> {
        data_memory_barrier();
        // Safety: Address valid, data barrier used.
        if unsafe { SPI1.add(STATUS).read_volatile() >> 10 & 1 == 1 } {
            return Err(hal_nb::nb::Error::WouldBlock);
        }
        // Safety: As above.
        unsafe { SPI1.add(IO).write_volatile(word as u32 | 8 << 24) };
        Ok(())
    }
}

/// One cannot use both this API and the `SpiBus` API at the same time. If needed, one should call
/// `clear_fifos` between APIs switches.
impl hal_nb::spi::FullDuplex<u16> for Spi1 {
    fn read(&mut self) -> hal_nb::nb::Result<u16, Self::Error> {
        data_memory_barrier();

        // Safety: Address valid, data barrier used.
        if unsafe { SPI1.add(STATUS).read_volatile() >> 7 & 1 == 1 } {
            return Err(hal_nb::nb::Error::WouldBlock);
        }

        // Safety: As above.
        Ok(unsafe { SPI1.add(IO).read_volatile() as u16 })
    }

    fn write(&mut self, word: u16) -> hal_nb::nb::Result<(), Self::Error> {
        data_memory_barrier();
        // Safety: Address valid, data barrier used.
        if unsafe { SPI1.add(STATUS).read_volatile() >> 10 & 1 == 1 } {
            return Err(hal_nb::nb::Error::WouldBlock);
        }

        // Safety: As above.
        unsafe { SPI1.add(IO).write_volatile(word as u32 | 16 << 24) };
        Ok(())
    }
}

impl Drop for Spi1 {
    fn drop(&mut self) {
        data_memory_barrier();
        let disable = 1 << 11;
        // Safety: Address valid, data barrier used.
        unsafe { SPI1.add(CONTROL0).write_volatile(disable) };
        critical_section::with(|_| {
            // Safety: As above.
            let aux_enables = unsafe { AUX_ENABLES.read_volatile() };
            // Safety: As above.
            unsafe { AUX_ENABLES.write_volatile(aux_enables & !0b10) };
        })
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataOutHold {
    H0 = 0,
    H1 = 1,
    H4 = 2,
    H7 = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CsHighTime(u8);
