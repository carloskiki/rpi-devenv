use std::{
    io::{copy, stdout, Write},
    time::Duration,
};

use serialport::{DataBits, FlowControl, Parity, StopBits};

fn main() {
    let mut args = std::env::args();
    args.next(); // Skip the command name
    let port_name = args.next().expect("a port name should be provided");
    let binary_name = args.next().expect("a binary file should be provided");

    println!("=> Connecting to the serial port `{port_name}` ...");
    println!("=> Each operation on the port times out after 60 seconds.");
    let mut port = serialport::new(port_name, 115200)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .flow_control(FlowControl::None)
        .stop_bits(StopBits::One)
        .timeout(Duration::from_secs(60))
        .open()
        .expect("the serial port provided should be valid");
    println!("=> Connected to the serial port.");

    println!("=> Waiting for the device to be ready ...");
    let mut ready_buf = [0u8; 1];
    port.read_exact(&mut ready_buf)
        .expect("should be able to read from the serial port");
    assert_eq!(
        ready_buf[0], 0xff,
        "The device needs to output 0xff to indicate it is ready"
    );
    println!("=> The device is ready.");

    println!("=> Sending the binary ...");
    let binary = std::fs::read(binary_name).expect("the binary file should be readable");
    let binary_size: u32 = binary
        .len()
        .try_into()
        .expect("the binary file should be less than 2^32 bytes");
    port.write_all(&binary_size.to_le_bytes())
        .expect("should be able to write to the serial port");
    port.write_all(&binary)
        .expect("should be able to write to the serial port");
    println!("=> The binary has been sent.");

    port.set_timeout(Duration::from_secs(5))
        .expect("should be able to set the timeout");
    let _ = copy(&mut port, &mut stdout());
    stdout().flush().expect("should be able to flush stdout");
    println!("=> The device has finished executing.");
}
