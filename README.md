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
- [ ] Have a chain loader.
- [ ] Setup interrupt handling.
- [ ] Have Async GPIO handling.
- [ ] Understand JTAG - to implement it.

Steps to follow:
- Have a working chain loader.
- Have a working JTAG debugger.
- Enable memory maps
- Enable CAS Atomic in the GPIO interface.

## Notes
- I need the MMU in order to use CAS (Compare and Swap) instructions.

- When running with QEMU, the AUX_ENABLES register is already enabled, so when we try to acquire the lock
    we fail and panic, which is why we couldn't see the uart output to stdio before. Interesting.
