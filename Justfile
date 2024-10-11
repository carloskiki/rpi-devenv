set dotenv-load
set shell := ["zsh", "-cu"]

# Build and copy to external drive
default: build copy eject

# Build the binary `kernel.img` file
build:
    cargo build --release
    rust-objcopy -S target/arm-none-eabihf/release/rpi -O binary kernel.img

# Copy the binary file to specified drive
copy:
    cp kernel.img /Volumes/$OUT_DRIVE

# Eject the specified drive
eject:
    diskutil eject $OUT_DRIVE

qemu BIN *EXTRA_ARGS:
    cargo build --bin {{BIN}}
    cp target/arm-none-eabihf/debug/{{BIN}} kernel.img
    qemu-system-arm -M raspi0 {{EXTRA_ARGS}} -kernel kernel.img 

bootloader:
    RUSTFLAGS="-C link-arg=-Tsrc/bin/link.x" cargo build --release --bin bootloader
    rust-objcopy -S target/arm-none-eabihf/release/bootloader -O binary bootloader.img
    cp bootloader.img /Volumes/$OUT_DRIVE
