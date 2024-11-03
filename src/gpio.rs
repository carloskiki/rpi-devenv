pub mod state;

use core::{
    cell::Cell,
    convert::Infallible,
    future::Future,
    ptr::{read_volatile, write_volatile},
    sync::atomic::{AtomicU32, Ordering},
    task::{Poll, Waker},
};

use bitvec::{order::Lsb0, view::BitView};
use critical_section::Mutex;
use embedded_hal::digital::{self, InputPin, OutputPin};
use embedded_hal_async::digital::Wait;
use state::{DetectState, Input, Output, PinType, Pull};

use crate::data_memory_barrier;

const FUNCTION_SELECT_BASE: *mut u32 = 0x20200000 as *mut u32;
const SET_BASE: *mut u32 = 0x2020001C as *mut u32;
const CLEAR_BASE: *mut u32 = 0x20200028 as *mut u32;
const LEVEL_BASE: *mut u32 = 0x20200034 as *mut u32;
const PULL_CONTROL: *mut u32 = 0x20200094 as *mut u32;
const PULL_SET_BASE: *mut u32 = 0x20200098 as *mut u32;
const DETECT_STATUS_BASE: *mut u32 = 0x20200040 as *mut u32;

static GPIO_SET: GpioSet = GpioSet::new();

type WakerCell = Mutex<Cell<Option<Waker>>>;
static WAKER_SET: [WakerCell; 54] = [const { Mutex::new(Cell::new(None)) }; 54];

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

    /// Returns true if it was successfully locked, false if it was already locked.
    fn lock<const PIN: u8>(&self) -> bool {
        if PIN < 32 {
            let mask = 1 << PIN;
            self.lower.fetch_or(mask, Ordering::AcqRel) & mask != 0
        } else {
            let mask = 1 << (PIN - 32);
            self.upper.fetch_or(mask, Ordering::AcqRel) & mask != 0
        }
    }

    /// Returns true if it was successfully unlocked, false if it was already unlocked.
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

impl<const PIN: u8, Ty> digital::ErrorType for Pin<PIN, Ty> {
    type Error = Infallible;
}

impl<const PIN: u8> OutputPin for Pin<PIN, Output> {
    fn set_high(&mut self) -> Result<(), Self::Error> {
        data_memory_barrier();
        // Safety: Both this address and the following one are valid for writing.
        // Memory barrier used.
        unsafe {
            double_register_op::<PIN, _, _>(SET_BASE, |addr, mask| {
                write_volatile(addr, mask);
            })
        }
        Ok(())
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        data_memory_barrier();
        // Safety: Both this address and the following one are valid for writing.
        // Memory barrier used.
        unsafe {
            double_register_op::<PIN, _, _>(CLEAR_BASE, |addr, mask| {
                write_volatile(addr, mask);
            })
        }
        Ok(())
    }
}

impl<const PIN: u8> InputPin for Pin<PIN, Output> {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        // Act as if we are an input pin, because we do have access to those registers even as an
        // output pin.
        Pin::<PIN, Input>::is_high(&mut Pin::<PIN, Input> {
            _pin: core::marker::PhantomData,
        })
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.is_high().map(|high| !high)
    }
}

impl<const PIN: u8> InputPin for &Pin<PIN, Output> {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        // Act as if we are an input pin, because we do have access to those registers even as an
        // output pin.
        Pin::<PIN, Input>::is_high(&mut Pin::<PIN, Input> {
            _pin: core::marker::PhantomData,
        })
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.is_high().map(|high| !high)
    }
}

impl<const PIN: u8> InputPin for &Pin<PIN, Input> {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        data_memory_barrier();
        // Safety: Both this address and the following one are valid for reading.
        // Memory barrier used.
        unsafe {
            double_register_op::<PIN, _, _>(LEVEL_BASE, |addr, mask| {
                let level = read_volatile(addr);
                Ok(level & mask != 0)
            })
        }
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.is_high().map(|high| !high)
    }
}

impl <const PIN: u8> InputPin for Pin<PIN, Input> {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        data_memory_barrier();
        // Safety: Both this address and the following one are valid for reading.
        // Memory barrier used.
        unsafe {
            double_register_op::<PIN, _, _>(LEVEL_BASE, |addr, mask| {
                let level = read_volatile(addr);
                Ok(level & mask != 0)
            })
        }
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        self.is_high().map(|high| !high)
    }
}

impl<const PIN: u8> Wait for Pin<PIN, Input> {
    fn wait_for_high(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        MapFuture {
            future: self
                .detect(DetectState::HIGH),
            map: |()| Ok(()),
        }
    }

    fn wait_for_low(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        MapFuture {
            future: self
                .detect(DetectState::LOW),
            map: |()| Ok(()),
        }
    }

    fn wait_for_rising_edge(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        MapFuture {
            future: self
                .detect(DetectState::RISING_EDGE),
            map: |()| Ok(()),
        }
    }

    fn wait_for_falling_edge(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        MapFuture {
            future: self
                .detect(DetectState::FALLING_EDGE),
            map: |()| Ok(()),
        }
    }

    fn wait_for_any_edge(&mut self) -> impl Future<Output = Result<(), Self::Error>> {
        MapFuture {
            future: self
                .detect(DetectState::RISING_EDGE | DetectState::FALLING_EDGE),
            map: |()| Ok(()),
        }
    }
}

impl<const PIN: u8> Pin<PIN, Input> {
    pub fn set_pull(&self, pull: Option<Pull>) {
        let pull = match pull {
            Some(Pull::Up) => 0b10,
            Some(Pull::Down) => 0b01,
            None => 0b00,
        };

        data_memory_barrier();
        // Safety: The address is valid for writing.
        // Memory barrier used.
        unsafe {
            write_volatile(PULL_CONTROL, pull);
        }

        // Wait 150 clock cycles according to manual p. 101.
        for _ in 0..150 {
            core::hint::spin_loop();
        }

        // Safety: Both this address and the following one are valid for writing.
        // Memory barrier used.
        unsafe {
            double_register_op::<PIN, _, _>(PULL_SET_BASE, |addr, mask| {
                write_volatile(addr, mask);
            })
        }

        // Wait another 150 clock cycles according to manual p. 101.
        for _ in 0..150 {
            core::hint::spin_loop();
        }

        // Safety: Both this address and the following one are valid for writing.
        // Memory barrier used.
        unsafe { write_volatile(PULL_CONTROL, 0) };
    }

    /// Returns a future that can be awaited, or can be blocked on by calling
    /// [`Detector::block()`].
    ///
    /// This could be made to not require mutable access in the future, but that would make the
    /// underlying implementation more memory consuming.
    pub fn detect(&mut self, state: DetectState) -> Detector<'_, PIN> {
        Detector {
            _pin: self,
            state,
            setup: false,
        }
    }
}

/// Detect an event on a GPIO pin.
///
/// You can either use this struct as a future, or call [`Detector::block()`] to block until the
/// event has occured.
///
/// If [`DetectState::empty()`] is used, the detector will immediately return.
pub struct Detector<'a, const PIN: u8> {
    _pin: &'a mut Pin<PIN, Input>,
    state: DetectState,
    setup: bool,
}

impl<const PIN: u8> Detector<'_, PIN> {
    // Block for the detection to occur.
    pub fn block(&mut self) {
        data_memory_barrier();
        // Safety: Memory barrier used.
        self.setup_detection();
        let first_reg = match self.state.registers().next() {
            Some(reg) => reg,
            None => return,
        };

        // Safety: The register is valid for reading.
        // Memory barrier used.
        while unsafe {
            // The interrupt will unset the detect state on all flags, so check when that is done.
            double_register_op::<PIN, _, _>(first_reg, |reg, mask| read_volatile(reg) & mask != 0)
        } {
            core::hint::spin_loop();
        }
    }

    fn setup_detection(&mut self) {
        data_memory_barrier();
        for register in self.state.registers() {
            // Safety: Both the register address and the next one are valid for writing.
            // Memory barrier used.
            unsafe {
                double_register_op::<PIN, _, _>(register, |reg, mask| {
                    critical_section::with(|_| {
                        let mut bits = read_volatile(reg);
                        bits |= mask;
                        write_volatile(reg, bits);
                    });
                });
            };
        }
        self.setup = true;
    }
}

impl<const PIN: u8> Future for Detector<'_, PIN> {
    type Output = ();

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        if self.state.is_empty() {
            // It is important that we do not set a waker because the interrupt routine will not be
            // called if no detection is set.
            return Poll::Ready(());
        }
        let waker = Some(cx.waker().clone());
        let slot = &WAKER_SET[PIN as usize];
        let mut old_waker = None;
        critical_section::with(|cs| {
            old_waker = slot.borrow(cs).replace(waker);
        });
        if !self.setup {
            self.setup_detection();
            return Poll::Pending;
        }

        if old_waker.is_none() {
            // The waker was taken by the interrupt routine, we are done.
            critical_section::with(|cs| {
                slot.borrow(cs).set(None);
            });
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
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

pub(crate) fn interrupt_handler1() {
    data_memory_barrier();
    // Safety: The status register is valid for reading.
    // Memory barrier used.
    let status = unsafe { read_volatile(DETECT_STATUS_BASE) };
    critical_section::with(|cs| {
        for bit_index in status.view_bits::<Lsb0>().iter_ones() {
            let waker = WAKER_SET[bit_index].borrow(cs).take();
            // At the cost of having a slower interrupt handler, we have a smaller waker set.
            // The tradeoff would be to store the `DetectState` in the waker set, but that would
            // make each slot 3 words instead of 2. But, we would not have iterate over all
            // registers.
            for register in DetectState::all().registers() {
                // Safety: We are looking at the first bank, so the offset to the base is 0.
                // Memory barrier used.
                unsafe {
                    let mut bits = read_volatile(register);
                    bits &= !(1 << bit_index);
                    write_volatile(register, bits);
                }
            }
            // Safety: The register is valid for writing, and we have cleared all interrupt sources
            // so we can clear the status and we know it wont stay set. A memory barrier is used.
            unsafe { write_volatile(DETECT_STATUS_BASE, 1 << bit_index) };

            if let Some(waker) = waker {
                waker.wake();
            }
        }
    });
}

pub(crate) fn interrupt_handler2() {
    data_memory_barrier();
    // Safety: The status register is valid for reading.
    // Memory barrier used.
    let status = unsafe { read_volatile(DETECT_STATUS_BASE.add(1)) };
    critical_section::with(|cs| {
        for bit_index in status.view_bits::<Lsb0>().iter_ones() {
            let waker = WAKER_SET[bit_index].borrow(cs).take();
            // At the cost of having a slower interrupt handler, we have a smaller waker set.
            // The tradeoff would be to store the `DetectState` in the waker set, but that would
            // make each slot 3 words instead of 2. But, we would not have iterate over all
            // registers.
            for register in DetectState::all().registers() {
                // Safety: We are looking at the first bank, so the offset to the base is 0.
                // Memory barrier used.
                unsafe {
                    let mut bits = read_volatile(register.add(1));
                    bits &= !(1 << bit_index);
                    write_volatile(register.add(1), bits);
                }
            }
            // Safety: The register is valid for writing, and we have cleared all interrupt sources
            // so we can clear the status and we know it wont stay set. A memory barrier is used.
            unsafe { write_volatile(DETECT_STATUS_BASE.add(1), 1 << bit_index) };

            if let Some(waker) = waker {
                waker.wake();
            }
        }
    });
}

pin_project_lite::pin_project! {
    struct MapFuture<F, M> {
        #[pin]
        pub future: F,
        pub map: M,
    }
}

impl<F, M, O> Future for MapFuture<F, M>
where
    F: Future,
    M: FnMut(F::Output) -> O,
{
    type Output = O;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let this = self.project();
        let output = core::task::ready!(this.future.poll(cx));
        Poll::Ready((this.map)(output))
    }
}
