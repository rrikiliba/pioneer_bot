use std::{io, thread};
use std::io::Write;
use std::time::Duration;
use rand::random;

fn main() {
    // uncomment the needed function

    read_from_serial();
    // write_to_serial();
}

#[allow(dead_code)]
fn read_from_serial() {
    match serialport::available_ports() {
        Ok(ports) => {
            for port in ports {
                if let serialport::SerialPortType::UsbPort(_) = port.port_type {
                    println!("Connecting to port {}", port.port_name);
                    let builder = serialport::new(port.port_name, 115_200);
                    if let Ok(mut port) = builder.open() {
                        println!("Connected!");

                        let mut buf = [0u8; 64];
                        loop {
                            match port.read(&mut buf) {
                                Ok(_) => println!("Received: {:?}", buf[0]),
                                Err(e) => {
                                    match e.kind() {
                                        io::ErrorKind::ConnectionAborted | io::ErrorKind::Interrupted => println!("Disconnected."),
                                        io::ErrorKind::TimedOut => println!("Connection timed out."),
                                        e => eprintln!("{e}"),
                                    }
                                    break;
                                }
                            }
                            thread::sleep(Duration::from_millis(10));
                        }
                    }
                }
            }
        }
        Err(_) => eprintln!("No open ports found")
    }
}

#[allow(dead_code)]
fn write_to_serial() {
    match serialport::available_ports() {
        Ok(ports) => {
            for port in ports {
                if let serialport::SerialPortType::UsbPort(_) = port.port_type {
                    println!("Connecting to port {}", port.port_name);
                    let builder = serialport::new(port.port_name, 115_200);
                    if let Ok(mut port) = builder.open() {
                        println!("Connected!");
                        let mut i = 0;
                        loop {
                            if i % 100 == 0 {
                                match port.write(&random::<f32>().to_le_bytes()) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        println!("{}", e.kind());
                                        break;
                                    }
                                }
                            }
                            i += 1;
                            thread::sleep(Duration::from_millis(10));
                        }
                    }
                }
            }
        }
        Err(_) => eprintln!("No open ports found")
    }
}