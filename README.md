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

## Memory Map of our Environment
Questions:
- Where do we put the stack?
- Do we need different stacks for all interrupt modes?
- Where do we put the heap?
- Where do we put the kernel?
- Where do we put the interrupt vector?

Claims:
- We want to be able to allocate some VC4 Memory.
- We want the stack to be resizeable.
- We want the heap to be "what's left over".

How do we set up the MMU?
- We want the stack to be protected (exception on overflow).
- So, what granularity do we want to offer? (1MiB is sensible). So "per section".
- We are loaded at some address, how do we compact memory as much as possible.

- What range of memory do we allow to be given to VC4?

### The Map

| 0x0000 Interupt Vectors |
| ... Nothing             |
| 0x8000 "boot" code      |
| ... data                |
| ... bss                 |
| ... Heap start          |
|   vvv Heap grows vvv    |
| ....................... |
| 0xXXX00000 Stack end    | (0x19900000 by default)
|   ^^^ Stack grows ^^^   |
| 0x20000000 Stack end    |



## TODOs
- [x] Have a chain loader.
- [x] Have a working MMU.
- [x] Understand JTAG - to implement it (implemented by default).
- [ ] Move the stack pointer and have a good memory layout.
- [ ] Setup interrupt handling.
- [ ] Have Async GPIO handling.
- [ ] Make it so that we don't need to fuckin change between `get` and `get_unchecked` for QEMU or the pi.

Steps to follow:
- Have a working chain loader.
- Have a working JTAG debugger.
- Enable memory maps
- Enable CAS Atomic in the GPIO interface.

## Notes
- I need the MMU in order to use CAS (Compare and Swap) instructions.
- When running with QEMU, the AUX_ENABLES register is already enabled, so when we try to acquire the lock
    we fail and panic, which is why we couldn't see the uart output to stdio before. Interesting.
- The theoretical maximal address for the chip is 0x20000000 (512MB)
- QEMU does not support supersections (16MB sections) in the translation table, so we need to use 1MB sections.

## The Bootloader

This is a very simple bootloader (or specifically a chain loader) that loads a binary from the Mini UART.

### Protocol

The bootloader will send a `0xff` byte to signal it is ready to receive the binary. The following four bytes
must be the size of the binary (in bytes) in little endian format. Upon receiving the binary, the bootloader
will output a success message to the UART, clean itself up, and jump to the binary.

### Bootcom

You can use the bootcom tool to send the binary to the bootloader.
