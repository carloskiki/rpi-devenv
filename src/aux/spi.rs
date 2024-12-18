use core::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    slice,
    task::{Context, Poll},
};

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
    hal, hal_async, hal_nb, set_waker, wake, WakerCell, WAKER_CELL_INIT,
};

use super::AUX_ENABLES;

pub mod mode;
mod registers;
use registers::{CONTROL0, CONTROL1, IO, SPI1, STATUS, TXHOLD};

static SPI_WAKER: WakerCell = WAKER_CELL_INIT;

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
            let entry = to_entry(chunk, ms_bit_first);

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

    // TODO: This could be optimized, but I can't be bothered.
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

        // Safety: Address valid, data memory barrier used.
        let out_ms_bit_first = unsafe { SPI1.add(CONTROL0).read_volatile() } >> 6 & 1 != 0;
        // Safety: As above.
        let in_ms_bit_first = unsafe { SPI1.add(CONTROL1).read_volatile() } & 1 != 0;
        let words_len = words.len();
        for (index, chunk) in words.chunks_mut(3).enumerate() {
            let entry = to_entry(chunk, out_ms_bit_first);

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
            from_entry(chunk, entry, in_ms_bit_first);
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        data_memory_barrier();
        // Safety: data barrier used.
        while unsafe { self.busy() } {
            core::hint::spin_loop();
        }
        self.clear_fifos();
        Ok(())
    }
}

impl hal_async::spi::SpiBus for Spi1 {
    fn read(&mut self, words: &mut [u8]) -> impl Future<Output = Result<(), Self::Error>> {
        <Self as hal_async::spi::SpiBus>::transfer_in_place(self, words)
    }

    async fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        hal_async::spi::SpiBus::flush(self).await?;

        WriteFut {
            spi: self,
            chunks: words.chunks(3),
        }
        .await
    }

    // TODO: This could be optimized, but I can't be bothered.
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

    async fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        hal_async::spi::SpiBus::flush(self).await?;

        TransferInPlaceFut {
            spi: self,
            words: as_chunks_mut(words),
            tx_index: 0,
            rx_index: 0,
        }
        .await
    }

    fn flush(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        FlushFut { spi: self }
    }
}

pub struct WriteFut<'a, 'b> {
    spi: &'a mut Spi1,
    chunks: slice::Chunks<'b, u8>,
}

impl Future for WriteFut<'_, '_> {
    type Output = Result<(), Infallible>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        data_memory_barrier();

        // Safety: Address valid, data memory barrier used.
        let ms_bit_first = unsafe { SPI1.add(CONTROL0).read_volatile() } >> 6 & 1 != 0;

        while unsafe { SPI1.add(STATUS).read_volatile() >> 10 & 1 } == 0 {
            let Some(chunk) = self.chunks.next() else {
                return Poll::Ready(Ok(()));
            };
            let entry = to_entry(chunk, ms_bit_first);

            if self.chunks.len() == 0 {
                // Safety: As above.
                unsafe { SPI1.add(IO).write_volatile(entry) };
                return Poll::Ready(Ok(()));
            } else {
                // Safety: As above.
                unsafe { SPI1.add(TXHOLD).write_volatile(entry) };
            }
        }

        critical_section::with(|cs| {
            set_waker(&SPI_WAKER, cx.waker(), cs);

            // Safety: Address valid, data barrier used, and we have exclusive access.
            unsafe {
                let reg = SPI1.add(CONTROL1).read_volatile();
                SPI1.add(CONTROL1).write_volatile(reg | 1 << 7);
            };
        });

        Poll::Pending
    }
}

pub struct TransferInPlaceFut<'a, 'b> {
    spi: &'a mut Spi1,
    // Guaranteed to be non-empty by the constructor.
    words: (&'b mut [[u8; 3]], &'b mut [u8]),
    tx_index: usize,
    rx_index: usize,
}

impl Future for TransferInPlaceFut<'_, '_> {
    type Output = Result<(), Infallible>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        data_memory_barrier();

        // Safety: Address valid, data memory barrier used.
        let out_ms_bit_first = unsafe { SPI1.add(CONTROL0).read_volatile() } >> 6 & 1 != 0;
        // Safety: As above.
        let in_ms_bit_first = unsafe { SPI1.add(CONTROL1).read_volatile() } & 1 != 0;

        while unsafe { SPI1.add(STATUS).read_volatile() >> 7 & 1 } == 0
            && self.rx_index <= self.words.0.len()
        {
            // Safety: Address valid, data memory barrier used.
            let entry = unsafe { SPI1.add(IO).read_volatile() };
            let rx_index = self.rx_index;
            if self.rx_index < self.words.0.len() {
                from_entry(&mut self.words.0[rx_index], entry, in_ms_bit_first);
            } else {
                from_entry(&mut self.words.1, entry, in_ms_bit_first);
            }
            self.rx_index += 1;
        } 
        if self.rx_index > self.words.0.len() {
            return Poll::Ready(Ok(()));
        }
        if self.tx_index > self.words.0.len() {
            critical_section::with(|cs| {
                set_waker(&SPI_WAKER, cx.waker(), cs);

                // Safety: Address valid, data barrier used, and we have exclusive access.
                unsafe {
                    let reg = SPI1.add(CONTROL1).read_volatile();
                    SPI1.add(CONTROL1).write_volatile(reg | 1 << 6);
                };
            });
            
            return Poll::Pending;
        }
        
        while unsafe { SPI1.add(STATUS).read_volatile() >> 10 & 1 } == 0
            && self.tx_index <= self.words.0.len()
        {
            let words = self.words.0.get(self.tx_index).map(|x| x.as_slice()).unwrap_or(self.words.1);
            let entry = to_entry(words, out_ms_bit_first);

            if self.tx_index < self.words.0.len() {
                // Safety: Address valid, data memory barrier used.
                unsafe { SPI1.add(TXHOLD).write_volatile(entry) };
            } else {
                // Safety: Address valid, data memory barrier used.
                unsafe { SPI1.add(IO).write_volatile(entry) };
            }
            self.tx_index += 1;
        }

        critical_section::with(|cs| {
            set_waker(&SPI_WAKER, cx.waker(), cs);

            // Safety: Address valid, data barrier used, and we have exclusive access.
            unsafe {
                let reg = SPI1.add(CONTROL1).read_volatile();
                SPI1.add(CONTROL1).write_volatile(reg | 1 << 7);
            };
        });

        Poll::Pending
    }
}

pub struct FlushFut<'a> {
    spi: &'a mut Spi1,
}

impl Future for FlushFut<'_> {
    type Output = Result<(), Infallible>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        data_memory_barrier();
        // Safety: data barrier used.
        if unsafe { self.spi.busy() } {
            critical_section::with(|cs| set_waker(&SPI_WAKER, cx.waker(), cs));
            // Safety: Address valid, data barrier used.
            unsafe {
                let reg = SPI1.add(CONTROL1).read_volatile();
                SPI1.add(CONTROL1).write_volatile(reg | 1 << 6);
            };

            Poll::Pending
        } else {
            self.spi.clear_fifos();
            Poll::Ready(Ok(()))
        }
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

fn to_entry(slice: &[u8], ms_bit_first: bool) -> u32 {
    let mut entry = 0;
    if ms_bit_first {
        for (i, byte) in slice.iter().enumerate() {
            entry |= (*byte as u32) << ((slice.len() - i) * 8);
        }
    } else {
        for (i, byte) in slice.iter().enumerate() {
            entry |= (*byte as u32) << (i * 8);
        }
    }

    entry |= (slice.len() as u32) << 24;
    entry
}

fn from_entry(slice: &mut [u8], entry: u32, ms_bit_first: bool) {
    let slice_len = slice.len();
    if ms_bit_first {
        for (i, byte) in slice.iter_mut().enumerate() {
            *byte = (entry >> ((slice_len - i) * 8)) as u8;
        }
    } else {
        for (i, byte) in slice.iter_mut().enumerate() {
            *byte = (entry >> (i * 8)) as u8;
        }
    }
}

// Unstable functions coming from the `std` lib.

#[inline]
pub const fn as_chunks_mut<const N: usize, T>(slice: &mut [T]) -> (&mut [[T; N]], &mut [T]) {
    const { assert!(N != 0, "chunk size must be non-zero") };
    let len_rounded_down = slice.len() / N * N;
    // SAFETY: The rounded-down value is always the same or smaller than the
    // original length, and thus must be in-bounds of the slice.
    let (multiple_of_n, remainder) = unsafe { slice.split_at_mut_unchecked(len_rounded_down) };
    // SAFETY: We already panicked for zero, and ensured by construction
    // that the length of the subslice is a multiple of N.
    let array_slice = unsafe { as_chunks_unchecked_mut(multiple_of_n) };
    (array_slice, remainder)
}

#[inline]
const unsafe fn as_chunks_unchecked_mut<const N: usize, T>(slice: &mut [T]) -> &mut [[T; N]] {
    // assert_unsafe_precondition!(
    //     check_language_ub,
    //     "slice::as_chunks_unchecked requires `N != 0` and the slice to split exactly into `N`-element chunks",
    //     (n: usize = N, len: usize = self.len()) => n != 0 && len % n == 0
    // );
    // SAFETY: Caller must guarantee that `N` is nonzero and exactly divides the slice length
    let new_len = slice.len() / N;
    // SAFETY: We cast a slice of `new_len * N` elements into
    // a slice of `new_len` many `N` elements chunks.
    unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr().cast(), new_len) }
}

pub(super) fn interrupt_handler() {
    data_memory_barrier();
    critical_section::with(|cs| {
        // Safety: Address is valid, data memory barrier used, we have exclusive access.
        unsafe {
            let reg = SPI1.add(CONTROL1).read_volatile();
            SPI1.add(CONTROL1).write_volatile(reg & !(0b11 << 6))
        };

        wake(&SPI_WAKER, cs);
    });
}
