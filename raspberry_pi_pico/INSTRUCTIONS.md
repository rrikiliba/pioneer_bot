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

finally, the main libraries used are
- rp2040-hal: abstraction layer for the peripherals of the RP2040
- ssd1306: easy to use driver to the ssd1306 OLED display
- usb-device and usbd-serial: to communicate via USB serial