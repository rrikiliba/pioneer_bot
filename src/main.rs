mod pilot;
mod pioneer_bot;

use pioneer_bot::PioneerBot;
use robotics_lib::runner::Runner;
use std::time::Duration;

use worldgen_unwrap::public::WorldgeneratorUnwrap;

fn main() {
    let pioneer_bot = PioneerBot::new(true);
    let mut world_generator = WorldgeneratorUnwrap::init(false, Some(std::path::PathBuf::from(r"world\\test_world")));
    let _continue_ = std::rc::Rc::clone(&pioneer_bot.running);
    if let Ok(mut runner) = Runner::new(Box::new(pioneer_bot), &mut world_generator) {
        'game: loop {
            if !*_continue_.borrow() {
                break 'game;
            }
            let _ = runner.game_tick();
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
