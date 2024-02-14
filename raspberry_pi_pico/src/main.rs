#![no_std]
#![no_main]

use core::fmt::Write;
use defmt_rtt as _;
use embedded_hal::adc::OneShot;
use panic_probe as _;
use rp2040_hal as hal;
use usbd_serial::SerialPort;

use embedded_hal::digital::v2::{InputPin, OutputPin};
use hal::{
    adc::{Adc, AdcPin},
    clocks::{init_clocks_and_plls, Clock},
    gpio, pac,
    sio::Sio,
    watchdog::Watchdog,
};
use rp2040_hal::entry;
use rp2040_hal::fugit::RateExtU32;
use ssd1306::{prelude::*, Ssd1306};
use usb_device::prelude::StringDescriptors;
use usb_device::LangID;

// necessary to boot to the rust binary
#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

// entry point for the program
#[entry]
fn main() -> ! {
    // setup of all peripherals and interfaces
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);
    let clocks = init_clocks_and_plls(
        12_000_000u32,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
        .ok()
        .unwrap();
    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());
    let pins = gpio::Pins::new(pac.IO_BANK0, pac.PADS_BANK0, sio.gpio_bank0, &mut pac.RESETS);
    let mut adc = Adc::new(pac.ADC, &mut pac.RESETS);
    let sda_pin = pins.gpio0.into_pull_up_input().into_function::<gpio::FunctionI2C>();
    let scl_pin = pins.gpio1.into_pull_up_input().into_function::<gpio::FunctionI2C>();
    let i2c = rp2040_hal::I2C::i2c0(
        pac.I2C0,
        sda_pin,
        scl_pin,
        400.kHz(),
        &mut pac.RESETS,
        &clocks.peripheral_clock,
    );
    let interface = ssd1306::I2CDisplayInterface::new(i2c);

    // set up the pins used on the controller

    // led pin
    let mut led = pins.gpio18.into_push_pull_output();

    // button pins
    let confirm_button = pins.gpio12.into_pull_down_input();

    let right_button = pins.gpio17.into_pull_down_input();
    let left_button = pins.gpio14.into_pull_down_input();
    let down_button = pins.gpio16.into_pull_down_input();
    let up_button = pins.gpio15.into_pull_down_input();

    // potentiometer pin
    let mut wheel = AdcPin::new(pins.gpio28.into_floating_input());

    // display with i2c interface
    let mut display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0).into_terminal_mode();
    display.init().unwrap();
    display.clear().unwrap();

    // greet the user while setting up usb
    let _ = write!(display, "\ndo_not_panic!()");

    // set up usb serial connection
    let usb_bus = usb_device::bus::UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));
    let mut serial = SerialPort::new(&usb_bus);
    let usb_config = StringDescriptors::new(LangID::EN)
        .manufacturer("do_not_panic!()")
        .product("robot_controller");
    let builder = usb_device::prelude::UsbDeviceBuilder::new(&usb_bus, usb_device::prelude::UsbVidPid(0x16c0, 0x27dd))
        .device_class(2)
        .strings(&[usb_config])
        .unwrap();
    let mut usb = builder.build();

    // blink the pins a couple times
    for _ in 0u8..3 {
        led.set_high().unwrap();
        delay.delay_ms(300);
        led.set_low().unwrap();
        delay.delay_ms(300);
    }
    display.clear().unwrap();

    // main program

    // save previous selection so that the display can be refreshed
    // only when necessary
    let mut prev_select = 0usize;
    let mut mode_select: u8;
    let mut robot_ready = false;

    // first phase
    // select game mode
    loop {
        // read 10 consecutive values from the potentiometer and get the
        // average value
        let mut read: u16 = 0;
        for _ in 0..=9 {
            let tmp: u16 = adc.read(&mut wheel).unwrap();
            read += tmp;
        }
        read = read / 10;

        // convert the value
        let select = read as usize / 420;
        mode_select = if select < 5 { 1 } else { 0 };

        if select != prev_select {
            prev_select = select;
            display.clear().unwrap();
            if robot_ready {
                let _ = write!(
                    display,
                    "\nMode:\n{}",
                    if mode_select == 1 { "assisted" } else { "manual" }
                );
            }
        }

        if usb.poll(&mut [&mut serial]) {
            let mut buf = [0u8; 4];
            if let Ok(_) = serial.read(&mut buf) {
                led.set_high().unwrap();
                robot_ready = true;
                let _ = write!(display, "\nChoose mode:");
            }
        }

        if robot_ready && confirm_button.is_high().unwrap() {
            if let Ok(_) = serial.write(&[mode_select]) {
                led.set_high().unwrap();
                break;
            }
        }

        delay.delay_ms(10);
        led.set_low().unwrap();
    }

    display.clear().unwrap();
    let _ = write!(display, "Choose action");

    let messages = if mode_select == 1 {
        [
            "No Choice",
            "Charge",
            "Sell Fish",
            "Sell Wood",
            "Sell Rocks",
            "Go Fishing",
            "Gather Wood",
            "Gather Rocks",
            "Deposit Gold",
            "Go Exploring",
        ]
    } else {
        [
            "Do Nothing",
            "Deposit Gold",
            "Sell Content",
            "Place Road",
            "Place Tent",
            "Pick Up Content",
            "",
            "",
            "",
            "",
        ]
    };

    let mut allow_input = if mode_select == 1 { false } else { true };

    loop {
        // read 10 consecutive values from the potentiometer and get the
        // average value, in order to stabilize the readings from my
        // apparently not so precise potentiometer
        let mut read: u16 = 0;
        for _ in 0..=9 {
            let tmp: u16 = adc.read(&mut wheel).unwrap();
            read += tmp;
        }
        read = read / 10;

        // convert the value to a range between 0 and 9;
        // 420 is a value obtained empirically
        let mut select = read as usize / 420;

        if mode_select == 1 && select > 9 {
            select = 9
        } else if mode_select == 0 && select > 5 {
            select = 5
        }

        if select != prev_select {
            prev_select = select;
            display.clear().unwrap();
            let _ = write!(display, "\n{select}. {}", messages[select]);
        }

        // check if the main program is reporting something
        if usb.poll(&mut [&mut serial]) {
            let mut buf = [0u8; 4];
            if let Ok(_) = serial.read(&mut buf) {
                led.set_high().unwrap();
                display.clear().unwrap();
                let msg = f32::from_le_bytes(buf);

                // the msg read is the score
                if msg >= 0f32 {
                    let _ = write!(display, "\nScore:\n{}", msg);
                }
                // the msg read is telling the pico to do something,
                // for now any negative value signals that the main program
                // is ready to receive input
                else {
                    allow_input = true;
                }
            }
        }

        // if the button is detected to be pressed and the main program is ready
        if confirm_button.is_high().unwrap() && allow_input {
            if let Ok(_) = serial.write(&[select as u8]) {
                led.set_high().unwrap();
                allow_input = false;
            }
        }

        if mode_select == 0 && allow_input && right_button.is_high().unwrap() {
            allow_input = false;
            if let Ok(_) = serial.write(&[6u8]) {
                led.set_high().unwrap();
            }
        } else if mode_select == 0 && allow_input && left_button.is_high().unwrap() {
            allow_input = false;
            if let Ok(_) = serial.write(&[7u8]) {
                led.set_high().unwrap();
            }
        } else if mode_select == 0 && allow_input && down_button.is_high().unwrap() {
            allow_input = false;
            if let Ok(_) = serial.write(&[8u8]) {
                led.set_high().unwrap();
            }
        } else if mode_select == 0 && allow_input && up_button.is_high().unwrap() {
            allow_input = false;
            if let Ok(_) = serial.write(&[9u8]) {
                led.set_high().unwrap();
            }
        }


        delay.delay_ms(10);
        led.set_low().unwrap();
    }
}
