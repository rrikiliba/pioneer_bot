mod pilot;
mod pioneer_bot;

use std::rc::Rc;
use pioneer_bot::PioneerBot;
use robotics_lib::runner::Runner;
use worldgen_unwrap::public::WorldgeneratorUnwrap;

// edit these to change settings
const USE_WORLD_GEN_GUI: bool = false;
const USE_GAME_GUI: bool = true;
const USE_SOUND: bool = true;


fn main() {
    let pioneer_bot = PioneerBot::new(USE_GAME_GUI, USE_SOUND);
    let mut world_generator = WorldgeneratorUnwrap::init(USE_WORLD_GEN_GUI, Some(std::path::PathBuf::from("world\\test_world")));
    let _continue_ = Rc::clone(&pioneer_bot.running);
    if let Ok(mut runner) = Runner::new(Box::new(pioneer_bot), &mut world_generator) {
        'game: loop {
            if !*_continue_.borrow() {
                break 'game;
            }
            let _ = runner.game_tick();
        }
    }
}
