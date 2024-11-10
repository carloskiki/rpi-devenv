use core::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    ptr::{read_volatile, write_volatile},
    task::{Context, Poll},
};

use crate::{
    aux::uart::registers::*,
    data_memory_barrier, eio, eio_async,
    gpio::{self, state::Alternate5},
    hal_nb, set_waker, Sealed, WakerCell, WAKER_CELL_INIT,
};

use super::Config;

const FIFO_SIZE: u32 = 8;

static WRITER_WAKER: WakerCell = WAKER_CELL_INIT;

pub struct Writer<P> {
    pub(super) _tx_pin: P,
}

impl<P: TxPin> Writer<P> {
    pub fn get(tx_pin: P, config: &Config) -> Option<Self> {
        data_memory_barrier();

        critical_section::with(|_| {
            // Safety: Address is valid, and a memory barrier is used. A new `Writer` instance is not
            // created if it is already activated in the tx bit. Critical section used so that
            // two threads do not race to acquire the lock.
            unsafe {
                // Check if receiver is enabled
                let control_reg = read_volatile(EXTRA_CONTROL_REG);
                if control_reg & 0b11 != 0 {
                    return None;
                }
                // Enable receiver
                write_volatile(EXTRA_CONTROL_REG, control_reg | 0b10);
                // Clear fifo
                write_volatile(INTERRUPT_ID_REG, 0b100);
            }
            Some(())
        })?;

        config.setup();

        Some(Self { _tx_pin: tx_pin })
    }
    
    /// Get the reader without checking if it is already in use.
    ///
    /// # Safety
    ///
    /// UB if the reader is already in use.
    pub unsafe fn get_unchecked(tx_pin: P, config: &Config) -> Self {
        data_memory_barrier();

        // Safety: Address is valid, and a memory barrier is used. A new `Reader` instance is not
        // created if the `Reader` already activated in the rx bit.
        unsafe {
            // Enable receiver
            let control_reg = read_volatile(EXTRA_CONTROL_REG);
            write_volatile(EXTRA_CONTROL_REG, control_reg | 0b10);
            // Clear fifo
            write_volatile(INTERRUPT_ID_REG, 0b100);
        }

        config.setup();

        Self { _tx_pin: tx_pin }
    }
}

impl<P> Drop for Writer<P> {
    fn drop(&mut self) {
        data_memory_barrier();

        critical_section::with(|_| {
            // Safety: Address is valid, and a memory barrier is used. Drops the `Writer`
            // lock, by disabling the transmitter. A critical section is used so that two threads
            // do not race to acquire the lock.
            unsafe {
                let control_reg = read_volatile(EXTRA_CONTROL_REG);
                write_volatile(EXTRA_CONTROL_REG, control_reg & !0b10);
            };
        })
    }
}

impl<P: TxPin> eio::ErrorType for Writer<P> {
    type Error = Infallible;
}

impl<P: TxPin> hal_nb::serial::ErrorType for Writer<P> {
    type Error = Infallible;
}

impl<P: TxPin> eio::Write for Writer<P> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }

        data_memory_barrier();
        let available = loop {
            // Safety: Address is valid, memory barrier used.
            let status = unsafe { EXTRA_STATUS_REG.read_volatile() };
            let transmit_fifo_level = (status >> 24) & 0b1111;
            let available = FIFO_SIZE - transmit_fifo_level;
            if available > 0 {
                break available;
            }
        };
        let to_write = buf.len().min(available as usize);
        for byte in buf.iter().take(to_write) {
            // Safety: Address is valid, memory barrier used.
            unsafe { IO_REG.write_volatile(*byte as u32) };
        }

        Ok(to_write)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        data_memory_barrier();
        // Safety: Address is valid, memory barrier used.
        while (unsafe { EXTRA_STATUS_REG.read_volatile() } << 8) & 1 == 0 {
            core::hint::spin_loop();
        }
        Ok(())
    }
}

impl<P: TxPin> eio::WriteReady for Writer<P> {
    fn write_ready(&mut self) -> Result<bool, Self::Error> {
        data_memory_barrier();
        // Safety: Address is valid, memory barrier used.
        Ok(unsafe { EXTRA_STATUS_REG.read_volatile() } & 0b10 != 0)
    }
}

impl<P: TxPin> hal_nb::serial::Write for Writer<P> {
    fn write(&mut self, word: u8) -> hal_nb::nb::Result<(), Self::Error> {
        // Safety: Address is valid, memory barrier used.
        let status = unsafe { EXTRA_STATUS_REG.read_volatile() };
        let fifo_full = status & 0b10 == 0;
        if fifo_full {
            return Err(hal_nb::nb::Error::WouldBlock);
        }
        // Safety: As above.
        unsafe { IO_REG.write_volatile(word as u32) };
        Ok(())
    }

    fn flush(&mut self) -> hal_nb::nb::Result<(), Self::Error> {
        data_memory_barrier();
        // Safety: Address is valid, memory barrier used.
        if (unsafe { EXTRA_STATUS_REG.read_volatile() } << 8) & 1 == 0 {
            return Err(hal_nb::nb::Error::WouldBlock);
        }
        Ok(())
    }
}

/// This implementation is cancel-safe.
impl<P: TxPin> eio_async::Write for Writer<P> {
    fn write(&mut self, buf: &[u8]) -> impl Future<Output = Result<usize, Self::Error>> {
        WriteFut { buf }
    }

    fn flush(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        FlushFut
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        let mut buf = buf;
        while !buf.is_empty() {
            match self.write(buf).await {
                Ok(0) => core::panic!("write() returned Ok(0)"),
                Ok(n) => buf = &buf[n..],
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

struct WriteFut<'a> {
    buf: &'a [u8],
}

impl Future for WriteFut<'_> {
    type Output = Result<usize, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        data_memory_barrier();
        // Safety: Address is valid, memory barrier used.
        let status = unsafe { EXTRA_STATUS_REG.read_volatile() };
        let transmit_fifo_level = (status >> 24) & 0b1111;
        let available = FIFO_SIZE - transmit_fifo_level;
        if available == 0 {
            critical_section::with(|cs| {
                set_waker(&WRITER_WAKER, cx.waker(), cs);
                // Safety: As above.
                let mut reg = unsafe { INTERRUPT_ENABLE_REG.read_volatile() };
                // We use 1110 because the errata says that bit 2 and 3 should be set to 1.
                reg |= 0b1110;
                // Safety: As above.
                unsafe { INTERRUPT_ENABLE_REG.write_volatile(reg) };
            });

            return Poll::Pending;
        }

        let to_write = self.buf.len().min(available as usize);
        for byte in self.buf.iter().take(to_write) {
            // Safety: Address is valid, memory barrier used.
            unsafe { IO_REG.write_volatile(*byte as u32) };
        }
        Poll::Ready(Ok(to_write))
    }
}

struct FlushFut;

impl Future for FlushFut {
    type Output = Result<(), Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        data_memory_barrier();
        // Safety: Address is valid, memory barrier used.
        if (unsafe { EXTRA_STATUS_REG.read_volatile() } << 8) & 1 == 0 {
            critical_section::with(|cs| {
                set_waker(&WRITER_WAKER, cx.waker(), cs);
                // Safety: As above.
                let mut reg = unsafe { INTERRUPT_ENABLE_REG.read_volatile() };
                // We use 1110 because the errata says that bit 2 and 3 should be set to 1.
                reg |= 0b1110;
                // Safety: As above.
                unsafe { INTERRUPT_ENABLE_REG.write_volatile(reg) };
            });
            return Poll::Pending;
        }
        Poll::Ready(Ok(()))
    }
}

pub(super) fn interrupt_handler() {
    critical_section::with(|cs| {
        // Safety: As above.
        let mut reg = unsafe { INTERRUPT_ENABLE_REG.read_volatile() };
        reg &= !0b10;
        // Safety: As above.
        unsafe { INTERRUPT_ENABLE_REG.write_volatile(reg) };

        if let Some(waker) = WRITER_WAKER.borrow(cs).take() {
            waker.wake()
        }
    });
}

/// Trait that represents [`gpio::Pin`]s that are valid for use as the Mini UART TX pin.
#[allow(private_bounds)]
pub trait TxPin: Sealed {}

impl TxPin for gpio::Pin<14, Alternate5> {}
impl TxPin for gpio::Pin<32, Alternate5> {}
impl TxPin for gpio::Pin<40, Alternate5> {}
