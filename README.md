# pioneer bot

The objective of the robot is to travel from town to town while exploring new lands,
gathering resources to sell (Rocks, Wood and Fish) to the local communities and deposit the profits 
in the bank

The robot chooses the best course of action upon waking up in the morning (Praying status),
tries to take on the selected tasks and then goes to sleep in its tent (JollyBlock) as night falls

The autopilot is mindful of things such as the weather (current and forecast) as
well as its own backpack and energy level

If that's not enough, at any point the user can intervene by plugging the 
raspberry pi pico (or any RP2040 board I suppose), programmed in rust to communicate 
via USB serial with the main program and interact with it

![image of the raspberry pi pico controller]()

The code for the pico can be found in the [raspberry_pi_pico](raspberry_pi_pico/INSTRUCTIONS.md) folder.
Upon plugging the device in, the user is prompted to choose between the following two modes:

## Assisted mode

the robot still functions on its own, but the user can intervene in the Praying phase, essentially 
suggesting what to do to for the day. If the robot can't carry out the selected operation, it will choose some better 
suitable course of action, as it would in autopilot

## Manual mode

the robot is fully controlled by the user via the makeshift controller. 4 additional buttons are added to it in order to 
collect user movement input, while the scroll wheel and confirmation button are still used to
select special actions like placing the tent or destroying content. I'm a little new to this and I couldn't manage to fit
them all on the same breadboard of the pico

![image of the added buttons]()

# Remarks

- The folder [serial_test](serial_test) contains a couple tests I used to check the USB functionality
- I will bring the raspberry pi pico to the oral exam, but I will not take the additional breadboard to show the manual mode, as it's
  - Not confirmed I will get it to work 100% (consider it a proof of concept in that case)
  - Really precarious to carry around as some of the components are loose
  - Difficult to set up on the spot 
  - Kind of boring overall, controlling the robot's movements could as easily be done via keyboard so I'd rather show off the other mode
- The .dll's in the root directory of the project alone don't allow the gui to work on windows; you need to place in

    `C:\Users\{user}\.rustup\toolchains\{toolchain}\lib\rustlib\{toolchain}\lib\`

    the .lib files contained in the latest releases at the following GitHubs:
  - [SDL2](https://github.com/libsdl-org/SDL)
  - [SDL2 Image](https://github.com/libsdl-org/SDL_image)
  - [SDL2 Text](https://github.com/libsdl-org/SDL_ttf)

  to clarify, these are all the files you should place in the directory stated above 
  - SDL2_ttf.lib
  - SDL2main.lib
  - SDL2test.lib
  - SDL2.lib
  - SDL2_image.lib