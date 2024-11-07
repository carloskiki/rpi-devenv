set dotenv-load
set shell := ["zsh", "-cu"]

# Build and copy to the root directory
out BIN="kernel": (build BIN) (copy BIN) eject

# Build the binary
build BIN="kernel":
    cd {{BIN}} && cargo build --release
    rust-objcopy -S target/armv6a-none-eabihf/release/{{BIN}} -O binary target/armv6a-none-eabihf/release/{{BIN}}.img

# Copy the binary file to specified drive
copy BIN="kernel":
    cp target/armv6a-none-eabihf/release/{{BIN}}.img /Volumes/$OUT_DRIVE/kernel.img

# Eject the specified drive
eject:
    diskutil eject $OUT_DRIVE

qemu BIN *EXTRA_ARGS:
    cd {{BIN}} && cargo build
    qemu-system-arm -M raspi0 {{EXTRA_ARGS}} -kernel target/armv6a-none-eabihf/debug/{{BIN}}

bootcom port BIN="kernel": (build BIN)
    cd ../bootcom && cargo r -- {{port}} ../rpi/target/armv6a-none-eabihf/release/{{BIN}}.img
