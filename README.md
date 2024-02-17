# pioneer bot

The objective of the robot is to travel from town to town while exploring new lands,
gathering resources to sell (Rocks, Wood and Fish) to the local communities and deposit the profits 
in the banks scattered in the settlements

The robot chooses the best course of action upon waking up in the morning (Praying status),
tries to take on the selected tasks and then goes to sleep in its tent (JollyBlock which it carries around) as night falls

The autopilot is mindful of things such as the weather (current and forecast) as
well as its own backpack and energy level

If that's not enough, at any point the user can intervene by plugging the 
raspberry pi pico (or any RP2040 board I suppose), programmed in rust to communicate 
via USB serial with the main program and interact with it

![image of the raspberry pi pico controller](raspberry_pi_pico/pics/remote.jpg)

The code for the pico can be found in the [raspberry_pi_pico](raspberry_pi_pico/) folder.
Upon plugging the device in, the user is prompted to choose between the following two modes:

## Assisted mode

This is the main mode. The robot still functions on its own, but the user can intervene in the Praying phase, essentially 
suggesting what to do to for the day (answering the robot's prayers). If the robot can't carry out the selected operation, it will choose some more 
suitable course of action, as it would in autopilot

## Manual mode

the robot is fully controlled by the user via the makeshift controller. 4 additional buttons are added to it in order to 
collect user movement input, while the scroll wheel and confirmation button are still used to
select special actions like placing the tent, destroying content or using the spyglass to explore tiles. I could not manage to fit
them all on the same breadboard of the pico, apologies for the unnecessary size of the thing

![image of the added buttons](raspberry_pi_pico/pics/controller.jpg)

# Remarks

- Beside the added functionalities, the main focus of the project, the AI, consists of the function `PioneerBot::auto_pilot(&mut self, world: &mut World, assisted: bool)`
- The folder [serial_test](serial_test) contains a couple of tests I used to check the USB functionality
- More info about the raspberry pi pico development can be found in [this file](raspberry_pi_pico/README.md)
- For the gui library to work on windows, you need to place the .lib files contained in the latest releases at the following GitHubs:
  - [SDL2](https://github.com/libsdl-org/SDL)
  - [SDL2 Image](https://github.com/libsdl-org/SDL_image)
  - [SDL2 Text](https://github.com/libsdl-org/SDL_ttf)

  in the directory: 
  `C:\Users\{user}\.rustup\toolchains\{toolchain}\lib\rustlib\{toolchain}\lib\`

  and include the respective .dll files in the root directory of the program

  I can't include or link directly to the correct files because the toolchain in use might be different among
  different machines, but to clarify, these are all the files you should place in the directory stated above 
  - SDL2_ttf.lib
  - SDL2main.lib
  - SDL2test.lib
  - SDL2.lib
  - SDL2_image.lib
  and these are the files that need to be placed inside `pioneer_bot`
  - SDL2.dll
  - SDL2_image.dll
  - SDL2_tff.dll

# Issues
- Sometimes the robot gets stuck when going to its destination. This is due to NLA compass not working properly, as we concluded whilst talking to the group that created it to try and fix it. I added some workarounds and checks, but they can only go so far in minimizing the issue
- When running the project, the startup time is considerable: this is due to the oxidizing agents' audio tool
  - The function `PioneerBot::new(gui_start: bool, audio_start: bool)` allows for both gui and sound effects to be disabled, if some quick testing is required