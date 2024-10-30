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

## TODOs
- [x] Have a chain loader.
- [x] Have a working MMU.
- [x] Understand JTAG - to implement it (implemented by default).
- [x] Map Undef, Data Abt, and Prefecth Abt to the panic handler.

- [x] Make it so that we don't need to fuckin change between `get` and `get_unchecked` for QEMU or the pi.
    Either: Use the lock but don't check with get_unchecked, or have a peripheral `init` that deactivates the miniuart
    when QEMU is used. We could also have a config flag that checks if the bin is compiled for QEMU.
- [x] Make a stack for ABORT mode and a stack for SVC.
- [x] Map the SYSTEM stack to a protected place in memory.
- [x] Make Undef use the ABORT cpu mode
- [x] Make IRQ use SVC mode
- [x] Test to make sure that SVC stack pointer always gets reset after interrupt handling.
    Why would it not? We pop everything from the stack in asm, and rust functions must not just indefinitely increase stack size.

- [x] Setup interrupt handling.
- [x] Have Async Timer handling.
- [x] Finish up the executor.
- [x] Test beliefs in interrupt handler.
- [ ] Async GPIO handling.
- [ ] Async MiniUart (Handle RTS/CTS using another rpi for com).

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

## The Bootloader

This is a very simple bootloader (or specifically a chain loader) that loads a binary from the Mini UART.

### Protocol

The bootloader will send a `0xff` byte to signal it is ready to receive the binary. The following four bytes
must be the size of the binary (in bytes) in little endian format. Then, the binary should be sent.
Upon receiving the binary, the bootloader will clean itself up, and jump to the binary.

### Bootcom

You can use the bootcom tool to send the binary to the bootloader.
