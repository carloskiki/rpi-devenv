use core::{
    future::Future,
    ptr::{read_volatile, write_volatile},
    slice::IterMut,
    task::Poll,
};

use embedded_io::ReadExactError;

use crate::{
    aux::uart::{registers::*, Config},
    data_memory_barrier, eio, eio_async,
    gpio::{self, state::Alternate5},
    hal_nb, set_waker, wake, Sealed, WakerCell, WAKER_CELL_INIT,
};

static READER_WAKER: WakerCell = WAKER_CELL_INIT;

#[derive(Debug)]
pub struct Reader<P> {
    pub(super) _rx_pin: P,
}

impl<P: RxPin> Reader<P> {
    pub fn get(rx_pin: P, config: &Config) -> Option<Self> {
        data_memory_barrier();

        critical_section::with(|_| {
            // Safety: Address is valid, and a memory barrier is used. A new `Reader` instance is not
            // created if the `Reader` already activated in the rx bit.  Critical section used so that
            // two threads do not race to acquire the lock.
            unsafe {
                // Check if receiver is enabled
                let control_reg = read_volatile(EXTRA_CONTROL_REG);
                if control_reg & 0b11 != 0 {
                    return None;
                }
                // Enable receiver
                write_volatile(EXTRA_CONTROL_REG, control_reg | 1);
                // Clear fifo
                write_volatile(INTERRUPT_ID_REG, 0b10);
            }
            Some(())
        })?;

        config.setup();

        Some(Self { _rx_pin: rx_pin })
    }

    /// Get the reader without checking if it is already in use.
    ///
    /// # Safety
    ///
    /// UB if the reader is already in use.
    pub unsafe fn get_unchecked(rx_pin: P, config: &Config) -> Self {
        data_memory_barrier();

        critical_section::with(|_| {
            // Safety: Address is valid, and a memory barrier is used.
            unsafe {
                // Enable receiver
                let control_reg = read_volatile(EXTRA_CONTROL_REG);
                write_volatile(EXTRA_CONTROL_REG, control_reg | 1);
                // Clear fifo
                write_volatile(INTERRUPT_ID_REG, 0b10);
            }
        });

        config.setup();

        Self { _rx_pin: rx_pin }
    }
}

impl<P> Drop for Reader<P> {
    fn drop(&mut self) {
        data_memory_barrier();

        critical_section::with(|_| {
            // Safety: Address is valid, and a memory barrier is used. Drops the `Reader`
            // lock, by disabling the receiver. A critical section is used so that two threads
            // do not race to acquire the lock.
            unsafe {
                let control_reg = read_volatile(EXTRA_CONTROL_REG);
                write_volatile(EXTRA_CONTROL_REG, control_reg & !1);
            };
        })
    }
}

impl<P: RxPin> eio::ErrorType for Reader<P> {
    type Error = Error;
}

impl<P: RxPin> hal_nb::serial::ErrorType for Reader<P> {
    type Error = Error;
}

impl<P: RxPin> eio::Read for Reader<P> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        data_memory_barrier();
        for (count, byte) in buf.iter_mut().enumerate() {
            loop {
                // Safety: Address is valid, data memory barrier used.
                let status_reg = unsafe { read_volatile(LINE_STATUS_REG) };
                if status_reg & 0b10 != 0 {
                    return Err(Error::Overrun);
                } else if status_reg & 1 != 0 {
                    // Safety: As above.
                    *byte = unsafe { read_volatile(IO_REG) as u8 };
                    break;
                } else if count != 0 {
                    return Ok(count);
                }
            }
        }
        Ok(buf.len())
    }

    fn read_exact(
        &mut self,
        buf: &mut [u8],
    ) -> Result<(), embedded_io::ReadExactError<Self::Error>> {
        data_memory_barrier();
        for byte in buf {
            loop {
                // Safety: Address valid, data memory barrier used.
                let status_reg = unsafe { read_volatile(LINE_STATUS_REG) };
                if status_reg & 0b10 != 0 {
                    return Err(Error::Overrun.into());
                } else if status_reg & 1 != 0 {
                    break;
                }
            }
            // Safety: As above.
            *byte = unsafe { read_volatile(IO_REG) as u8 };
        }
        Ok(())
    }
}

impl<P: RxPin> eio::ReadReady for Reader<P> {
    fn read_ready(&mut self) -> Result<bool, Self::Error> {
        data_memory_barrier();
        // Safety: Address is valid, data memory barrier used.
        Ok(unsafe { EXTRA_STATUS_REG.read_volatile() & 1 != 0 })
    }
}

impl<P: RxPin> hal_nb::serial::Read for Reader<P> {
    fn read(&mut self) -> hal_nb::nb::Result<u8, Self::Error> {
        data_memory_barrier();
        // Safety: Address is valid, data memory barrier used.
        let status_reg = unsafe { read_volatile(LINE_STATUS_REG) };
        if status_reg & 0b10 != 0 {
            return Err(hal_nb::nb::Error::Other(Error::Overrun));
        } else if status_reg & 1 != 0 {
            // Safety: As above.
            return Ok(unsafe { read_volatile(IO_REG) as u8 });
        }
        Err(hal_nb::nb::Error::WouldBlock)
    }
}

/// This implementation is cancel-safe.
///
/// Because of the way the Mini UART works, using `read` asynchronously will almost always read
/// only one byte. It is more efficient to use `read_exact` instead.
impl<P: RxPin> eio_async::Read for Reader<P> {
    fn read(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>> {
        ReadFut { _reader: self, buf }
    }

    fn read_exact(
        &mut self,
        buf: &mut [u8],
    ) -> impl Future<Output = Result<(), ReadExactError<Self::Error>>> {
        ReadExactFut {
            _reader: self,
            buf_iter: buf.iter_mut(),
        }
    }
}

#[derive(Debug)]
pub struct ReadFut<'a, 'b, P> {
    _reader: &'a mut Reader<P>,
    buf: &'b mut [u8],
}

impl<P: RxPin> Future for ReadFut<'_, '_, P> {
    type Output = Result<usize, Error>;

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context,
    ) -> Poll<Self::Output> {
        data_memory_barrier();
        for (count, byte) in self.buf.iter_mut().enumerate() {
            // Safety: Address is valid, data memory barrier used.
            let status_reg = unsafe { read_volatile(LINE_STATUS_REG) };
            if status_reg & 0b10 != 0 {
                return Poll::Ready(Err(Error::Overrun));
            }

            if status_reg & 1 != 0 {
                // Safety: As above.
                *byte = unsafe { read_volatile(IO_REG) as u8 };
            } else if count != 0 {
                return Poll::Ready(Ok(count));
            } else {
                critical_section::with(|cs| {
                    set_waker(&READER_WAKER, cx.waker(), cs);
                    // Safety: As above.
                    let mut reg = unsafe { INTERRUPT_ENABLE_REG.read_volatile() };
                    // We use 1101 because the errata says that bit 2 and 3 should be set to 1.
                    reg |= 0b1101;
                    // Safety: As above.
                    unsafe { INTERRUPT_ENABLE_REG.write_volatile(reg) };
                });
                return Poll::Pending;
            }
        }
        Poll::Ready(Ok(self.buf.len()))
    }
}

#[derive(Debug)]
pub struct ReadExactFut<'a, 'b, P> {
    _reader: &'a mut Reader<P>,
    buf_iter: IterMut<'b, u8>,
}

impl<P: RxPin> Future for ReadExactFut<'_, '_, P> {
    type Output = Result<(), ReadExactError<Error>>;

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        data_memory_barrier();
        for byte in self.buf_iter.by_ref() {
            // Safety: Address is valid, data memory barrier used.
            let status_reg = unsafe { read_volatile(LINE_STATUS_REG) };
            if status_reg & 0b10 != 0 {
                return Poll::Ready(Err(ReadExactError::Other(Error::Overrun)));
            }

            if status_reg & 1 == 0 {
                critical_section::with(|cs| {
                    set_waker(&READER_WAKER, cx.waker(), cs);
                    // Safety: As above.
                    let mut reg = unsafe { INTERRUPT_ENABLE_REG.read_volatile() };
                    // We use 1101 because the errata says that bit 2 and 3 should be set to 1.
                    reg |= 0b1101;
                    // Safety: As above.
                    unsafe { INTERRUPT_ENABLE_REG.write_volatile(reg) };
                });
                return Poll::Pending;
            }
            // Safety: As above.
            *byte = unsafe { read_volatile(IO_REG) as u8 };
        }
        Poll::Ready(Ok(()))
    }
}

/// Safety: Must be called only from the interrupt handler.
pub(super) unsafe fn interrupt_handler() {
    data_memory_barrier();
    critical_section::with(|cs| {
        // Safety: Address is valid, data memory barrier used.
        let mut reg = unsafe { INTERRUPT_ENABLE_REG.read_volatile() };
        reg &= !0b1;
        // Safety: As above.
        unsafe { INTERRUPT_ENABLE_REG.write_volatile(reg) };

        wake(&READER_WAKER, cs);
    });
}

/// Trait that represents [`gpio::Pin`]s that are valid for use as the MiniUART RX pin.
#[allow(private_bounds)]
pub trait RxPin: Sealed {}

// See the BCM2835 manual section 6.2 for the pin mappings.
impl RxPin for gpio::Pin<15, Alternate5> {}
impl RxPin for gpio::Pin<33, Alternate5> {}
impl RxPin for gpio::Pin<41, Alternate5> {}

/// Errors that can occurs when reading from the Mini UART.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Error {
    Overrun,
}

impl eio::Error for Error {
    fn kind(&self) -> embedded_io::ErrorKind {
        eio::ErrorKind::Other
    }
}

impl hal_nb::serial::Error for Error {
    fn kind(&self) -> hal_nb::serial::ErrorKind {
        hal_nb::serial::ErrorKind::Overrun
    }
}
