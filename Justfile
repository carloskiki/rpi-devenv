set dotenv-load
set shell := ["zsh", "-cu"]

# Build and copy to external drive
default BIN="kernel": (build BIN) (copy-img BIN)

# Build the binary
build BIN="kernel":
    cd {{BIN}} && cargo build --release

# Copy the executable as a binary file to the root of the project
copy-img BIN="kernel":
    rust-objcopy -S target/arm-none-eabihf/release/{{BIN}} -O binary {{BIN}}.img
    
# Copy the binary file to specified drive
copy-out BIN="kernel":
    cp {{BIN}}.img /Volumes/$OUT_DRIVE

# Eject the specified drive
eject:
    diskutil eject $OUT_DRIVE

qemu BIN *EXTRA_ARGS:
    cd {{BIN}} && cargo build
    qemu-system-arm -M raspi0 {{EXTRA_ARGS}} -kernel target/arm-none-eabihf/debug/{{BIN}}
