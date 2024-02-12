use crate::pioneer_bot::Objective;
use serialport::{ErrorKind, SerialPort};
use std::io;
use std::io::{Read, Write};

// support struct in order to interface the main program with the raspberry pi pico
pub struct Pilot {
    manual: bool,
    port: Box<dyn SerialPort>,
}

impl Pilot {
    // constructs a new pilot, only if the pico is connected to the machine this runs on
    pub fn new() -> Result<Pilot, serialport::Error> {
        match serialport::available_ports() {
            | Ok(ports) => {
                for port in ports {
                    if let serialport::SerialPortType::UsbPort(_) = port.port_type {
                        print!("Connecting pilot to port {}...", port.port_name);
                        let builder = serialport::new(port.port_name, 115_200);
                        return builder.open().map(|mut p| {
                            println!("Pilot connected! choose mode:");
                            let mut buf = [0];
                            let _ = p.write(&[0u8]);
                            loop {
                                if p.read(&mut buf).is_ok() {
                                    break;
                                }
                            }

                            return Pilot {
                                manual: buf[0] == 0,
                                port: p,
                            };
                        });
                    }
                }
                return Err(serialport::Error::new(ErrorKind::NoDevice, "Input device not found"));
            }
            | Err(e) => {
                return Err(e);
            }
        }
    }

    // write the score to the serial port
    pub(crate) fn put_score(&mut self, score: f32) {
        let _ = self.port.write(&score.to_le_bytes());
    }

    pub(crate) fn get_objective(&mut self) -> Result<Objective, ()> {
        // send the signal that an objective must be selected
        if let Ok(_) = self.port.write(&(-1.0f32).to_le_bytes()) {
            let mut buf = [0];
            loop {
                match self.port.read(&mut buf) {
                    | Ok(_) => break Ok(Objective::from(buf[0])),
                    | Err(e) => {
                        match e.kind() {
                            | io::ErrorKind::ConnectionAborted | io::ErrorKind::Interrupted => {
                                println!("Pilot disconnected.")
                            }
                            | io::ErrorKind::TimedOut => println!("Pilot connection timed out."),
                            | e => eprintln!("{e}"),
                        }
                        break Err(());
                    }
                }
            }
        } else {
            Err(())
        }
    }

    pub fn is_manual(&self) -> bool {
        self.manual
    }

    pub fn get_action(&mut self) -> i8 {
        let mut buf = [0];
        match self.port.read(&mut buf) {
            | Ok(_) => buf[0] as i8,
            | Err(e) => match e.kind() {
                | io::ErrorKind::ConnectionAborted | io::ErrorKind::Interrupted => {
                    println!("Pilot disconnected.");
                    -1
                }
                | io::ErrorKind::TimedOut => {
                    println!("Pilot connection timed out.");
                    -1
                }
                | e => {
                    println!("{e}");
                    0
                }
            },
        }
    }
}
