# Raspberry Pi Zero Env

## Sources
- [Great ARM tutorial for Raspberry Pi](https://github.com/BrianSidebotham/arm-tutorial-rpi). This
    tutorial provides a good overview of how to write a simple operating system for the Raspberry Pi.
- [Rasperry Pi Zero bare metal examples](https://github.com/dwelch67/raspberrypi-zero). Simple examples for
    the Raspberry Pi zero.
- [Raspberry Pi Bare Metal Forum](https://forums.raspberrypi.com/viewtopic.php?t=72260). The official forum
    for any Raspberry Pi bare metal stuff.
- [BCM2835 ARM Peripherals](https://www.raspberrypi.org/app/uploads/2012/02/BCM2835-ARM-Peripherals.pdf). This
    document provides a good overview of the BCM2835 SoC, which is used in the Raspberry Pi zero v1.3.
- [ARM1176JZF-S Technical Reference Manual](https://developer.arm.com/documentation/ddi0301/h). This processor
    is an ARM11 (ARMv6) processor, which powers the Raspberry Pi zero v1.3.
- [ARMv5/ARMv6 Reference Manual](ARMv5-ARM.pdf). This manual is for the ARMv5 and ARMv6 architectures.
- [BCM2835 System Timer Wiki](https://xinu.cs.mu.edu/index.php/BCM2835_System_Timer). This wiki page
    provides a good overview of the BCM2835 System Timer, which is used in the Raspberry Pi zero v1.3.
- [Embedded Xinu](https://embedded-xinu.readthedocs.io/en/latest/Introduction.html) A great research implementation
    on the BCM2835 and more ...
- [Linker Script mcyoung](https://mcyoung.xyz/2021/06/01/linker-script/) A great blog post about linker scripts
    and how they work.
- [Linux Insides](https://0xax.gitbooks.io/linux-insides/content/index.html) A great book about the Linux kernel
    and how it works.

## GPU Sources
- [All about accelerated video on the Raspberry Pi](https://forums.raspberrypi.com/viewtopic.php?t=317511)
- [drm/vc4 Broadcom VC4 Graphics Driver](https://www.kernel.org/doc/html/latest/gpu/vc4.html)

## TODOs
### Aux Interface
#### Uart
- [ ] Sending break signals
- [ ] Auto-flow control
- [ ] Manual RTS/CTS handles
- [ ] RTS/CTS control flow tied with HAL implementation

Auto-flow:
- await for CTS before sending data.
- De-assert RTS when we do not have space in the receive FIFO.

Also propose:
- RTS handling for transmitter.
- CTS handling for receiver.

__Tests:__
- [ ] Check if having pull-up/pull-down matters when pins are in alt mode (e.g., MiniUart)
- [ ] Check if reading from `set` and `clear` GPIO registers gives the state of the pin.

- [ ] Async MiniUart (Handle RTS/CTS using another rpi for com).
- [ ] Have a working bootloader - If we lose the SD card we are screwed.
- [ ] Have a test framework for QEMU, that should also be able to run on the PI.
- [ ] Driver implementation for all components.
- [ ] Do we need a heap?

## Future Things
- [ ] Enable FIQ
- [ ] Have Vectored IRQs and FIQs
- [ ] Have a variable stack size
- [ ] Make sure all caches are enabled
- [ ] Have some TLB locks (maybe?)
- [ ] Have a processor abort hook that has specific structs as input (instead of "str" for panic).

## Notes
- I need the MMU in order to use CAS (Compare and Swap) instructions.
- When running with QEMU, the AUX_ENABLES register is already enabled, so when we try to acquire the lock
    we fail and panic, which is why we couldn't see the uart output to stdio before. Interesting.
- The theoretical maximal address for the chip is 0x20000000 (512MB)
- QEMU does not support supersections (16MB sections) in the translation table, so we need to use 1MB sections.

- In the actual hardware, the memory is not capped at 0x20000000, does not look mirrored either so I don't know what
    is going on there.

## Mini UART Interesting Registers
IER:
- Receive interrupt: assert when at least 1 byte in FIFO.
- TRANSMIT interrupt: assert when transmit FIFO is empty.

LCR:
- Break: Assert break on TX line.

MCR:
- RTS (only when not auto-flow): Assert RTS line.

MSR:
- CTS (only when not auto-flow): Assert CTS line.

LSR:
- Receiver Overrun: If we lost some bytes when receiving.

CNTL:
- Various RTS/CTS settings.
- Auto-flow control.

## The Bootloader

This is a very simple bootloader (or specifically a chain loader) that loads a binary from the Mini UART.

### Protocol

The bootloader will send a `0xff` byte to signal it is ready to receive the binary. The following four bytes
must be the size of the binary (in bytes) in little endian format. Then, the binary should be sent.
Upon receiving the binary, the bootloader will clean itself up, and jump to the binary.

### Bootcom

You can use the bootcom tool to send the binary to the bootloader.
