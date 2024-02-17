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

                            println!("chosen {} mode", if buf[0] == 0 { "manual" } else { "assisted" });

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

    // since the pico updates every 10 ms I needed to take some measures to
    // inhibit double (or more) input
    pub fn get_action(&mut self) -> i8 {
        let mut buf = [0];
        match self.port.read(&mut buf) {
            | Ok(_) => {
                let mut input = buf[0] as i8;
                // signal to the pico that the program is ready to receive input,
                // this way buttons pressed when not needed aren't registered
                // (signaled on the pico by the led not lighting up)
                if self.port.write(&(-1f32).to_le_bytes()).is_ok() {

                    // I tried everything but couldn't cancel out the double input,
                    // so even though it's not good practice at all, I try to
                    // ignore a subsequent equal input every time one is received
                    if self.port.read(&mut buf).is_ok() && self.port.write(&(-2f32).to_le_bytes()).is_ok() {
                        if input != buf[0] as i8 { input = buf[0] as i8 }
                    }

                    // at first I was trying to clear the port's buffer after receiving the first input,
                    // but this didn't seem to work
                    // self.port.clear(ClearBuffer::Input);
                } else {
                    eprintln!("Write error");
                    input = -1;
                }
                input
            }
            | Err(e) => {
                match e.kind() {
                    | io::ErrorKind::ConnectionAborted | io::ErrorKind::Interrupted => {
                        println!("Pilot disconnected.");
                    }
                    | io::ErrorKind::TimedOut => {
                        println!("Pilot connection timed out.");
                    }
                    | e => {
                        println!("{e}");
                    }
                }
                -1
            }
        }
    }
}
