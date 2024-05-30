set dotenv-load
set shell := ["zsh", "-cu"]

# Build and copy to external drive
default: build copy eject

# Build the binary `kernel.img` file
build:
    cargo build --release
    rust-objcopy target/arm-none-eabihf/release/rpi -O binary kernel.img

# Copy the binary file to specified drive
copy:
    cp kernel.img /Volumes/$OUT_DRIVE

# Eject the specified drive
eject:
    diskutil eject $OUT_DRIVE

qemu: build
    qemu-system-arm -m 512 -M raspi0 -serial stdio -kernel kernel.img
