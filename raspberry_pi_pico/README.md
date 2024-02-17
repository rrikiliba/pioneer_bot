# Info and Instructions

this project is designed to run on a raspberry pi pico; to compile it, you will need the correct toolchain.
you can obtain it via rustup as follows

`rustup target add thumbv6m-none-eabi`

also install the flip-link linker and write the appropriate flags in the [cargo config file](.cargo/config.toml)

`cargo install flip-link --locked`

I used the crate elf2uf2-rs to get the executable inb the file format supported by the BOOTSEL mode of the pico
install it with

`cargo install elf2uf2-rs --locked`

after all that, make sure to include the memory.x file in the root directory of the project and
add the final clause to the [cargo config file](.cargo/config.toml) in order to let `cargo run` flash
the binary directly to the pico

# Libraries Used

- rp2040-hal: abstraction layer for the peripherals of the RP2040
- ssd1306: easy to use driver to the ssd1306 OLED display
- usb-device and usbd-serial: to communicate via USB serial

# Issues
- The program for the raspberry pi pico contains **a lot** of boilerplate code required by the hardware abstraction layer
- The manual mode suffers from a double input issue, where the buttons press gets registered twice on the pico. I have determined this issue to be completely on the hardware's side and I do not have the knowledge to fix it at this time
    - for the sake of showing a working example, I added a check for double input on the software side, which works due to the fact that the buttons **always** produce a double input
- The serial port interfaces for the main program and the pico environment have one important difference, the former is blocking while the latter isn't, and I couldn't find a way to make them both non-blocking
  - This means that when in manual mode, the world and the gui don't update unless an input is provided, which was not the intended behaviour when I started the project