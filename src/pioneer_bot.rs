use rand::random;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::mem;
use std::rc::Rc;

use robotics_lib::energy::Energy;
use robotics_lib::utils::LibError;
use robotics_lib::runner::Runnable;
use robotics_lib::event::events::Event;
use robotics_lib::interface::Direction::{Down, Left, Right, Up};
use robotics_lib::interface::{craft, destroy, get_score, go, look_at_sky, put, robot_map, robot_view, Direction};

use robotics_lib::runner::backpack::BackPack;
use robotics_lib::runner::Robot;
use robotics_lib::world::coordinates::Coordinate;
use robotics_lib::world::environmental_conditions::{DayTime, WeatherType};
use robotics_lib::world::tile::{Content, Tile};
use robotics_lib::world::World;

use another_one_bytes_the_dust_tile_resource_mapper_tool::tool::tile_mapper::TileMapper as Map;
use ohcrab_weather::weather_tool::{WeatherPredictionTool as Forecast, WeatherToolError};
use oxagaudiotool::{OxAgAudioTool, sound_config::OxAgSoundConfig};
use pmp_collect_all::CollectAll;
use rustbeef_nlacompass::compass::{Destination, MoveError, NLACompass as Compass};
use spyglass::spyglass::*;

use crate::pilot::Pilot;
use crate::pioneer_bot::Objective::{Charging, Depositing, Exploring, Gathering, Moving, Praying, Selling, Sleeping, Waiting};
use colored::{Color, Colorize};
use robo_gui::MainState;
use robotics_lib::world::tile::Content::JollyBlock as Tent;

// Possible states of the robot
#[derive(Clone, Debug, PartialEq)]
pub enum Objective {
    Waiting(DayTime),
    Moving(bool),
    Charging(usize),
    Sleeping,
    Praying,
    Gathering(Content),
    Selling(Content),
    Depositing,
    Exploring,
    None,
}

impl Display for Objective {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                | Waiting(d) => format!("waiting till {:?}", d),
                | Moving(_) => "moving".to_string(),
                | Charging(n) => format!("charging to {}/1000", n),
                | Sleeping => "sleeping".to_string(),
                | Praying => "praying".to_string(),
                | Gathering(_) => "gathering".to_string(),
                | Selling(_) => "selling".to_string(),
                | Depositing => "going to the bank".to_string(),
                | Exploring => "exploring".to_string(),
                | Objective::None => "doing nothing".to_string(),
            }
        )
    }
}

// main robot struct
pub struct PioneerBot<'a> {
    // Robot instance
    robot: Robot,
    // Interface to usb serial
    pilot: Option<Pilot>,

    // current score, updated each tick
    score: f32,
    // current and next objective set for the robot
    objective: Objective,
    next: Objective,

    // Tile Resource mapper to keep track of content discovered
    map: Map,

    // save some locations: ones that have already been explored and ones with depleted markets and banks
    pins: HashSet<(usize, usize)>,
    bankrupt: HashSet<(usize, usize)>,

    // NLA compass
    compass: Compass,
    // oh_crab weather tool
    forecast: Forecast,

    // audio and visuals
    audio: Option<OxAgAudioTool>,
    sounds: Option<Vec<OxAgSoundConfig>>,
    gui: Option<MainState<'a>>,

    last_coords: Vec<(usize, usize)>,
    pub(crate) running: Rc<RefCell<bool>>,
}

impl PioneerBot<'_> {
    // initialize the robot, you can choose to use gui, audio or both from src/main.rs
    pub fn new(gui_start: bool, audio_start: bool) -> Self {
        Self {
            robot: Default::default(),
            pilot: Pilot::new().ok(),

            score: 0.,
            objective: Objective::None,
            next: Objective::None,

            map: Map {},
            pins: HashSet::new(),
            bankrupt: HashSet::new(),

            compass: Compass::new(),
            forecast: Forecast::new(),

            audio: if audio_start {
                let mut audio_map = HashMap::new();
                audio_map.insert(
                    WeatherType::Sunny,
                    OxAgSoundConfig::new_looped_with_volume("sounds\\WeatherType\\Sunny.mp3", 0.25),
                );
                audio_map.insert(
                    WeatherType::Rainy,
                    OxAgSoundConfig::new_looped_with_volume("sounds\\WeatherType\\Rainy.mp3", 0.25),
                );
                audio_map.insert(
                    WeatherType::Foggy,
                    OxAgSoundConfig::new_looped_with_volume("sounds\\WeatherType\\Foggy.mp3", 0.25),
                );
                audio_map.insert(
                    WeatherType::TrentinoSnow,
                    OxAgSoundConfig::new_looped_with_volume("sounds\\WeatherType\\Snow.mp3", 0.25),
                );
                audio_map.insert(
                    WeatherType::TropicalMonsoon,
                    OxAgSoundConfig::new_looped_with_volume("sounds\\WeatherType\\Monsoon.mp3", 0.25),
                );

                let mut event_map = HashMap::new();
                event_map.insert(
                    Event::AddedToBackpack(Content::None, 0),
                    OxAgSoundConfig::new_with_volume("sounds\\Event\\Inventory.wav", 0.5),
                );

                OxAgAudioTool::new(event_map, Default::default(), audio_map).ok()
            } else {
                None
            },
            sounds: if audio_start {
                let mut sounds = Vec::new();
                sounds.push(OxAgSoundConfig::new_with_volume("sounds\\Effects\\objective.wav", 0.5));
                sounds.push(OxAgSoundConfig::new_with_volume("sounds\\Effects\\oof.wav", 0.5));
                sounds.push(OxAgSoundConfig::new_with_volume("sounds\\Effects\\coins.wav", 0.5));
                sounds.push(OxAgSoundConfig::new_with_volume("sounds\\Effects\\quackable.wav", 0.5));
                sounds.push(OxAgSoundConfig::new_with_volume("sounds\\Event\\Move.wav", 0.2));
                Some(sounds)
            } else { None },
            gui: if gui_start {
                Some(MainState::new(1).unwrap())
            } else { None },
            last_coords: Vec::new(),
            running: Rc::new(RefCell::new(true)),
        }
    }

    // wrapper function to get the coordinates directly as (usize, usize)
    // for the sake of compatibility
    fn get_coordinate_usize(&self) -> (usize, usize) {
        let coordinate = self.get_coordinate();
        (coordinate.get_row(), coordinate.get_col())
    }

    // sets the objective for the robot and logs it in the terminal
    pub(crate) fn set_objective(&mut self, objective: Objective) {
        self.objective = objective;
        if let (Some(audio), Some(sounds)) = (self.audio.as_mut(), self.sounds.as_ref()) {
            let _ = audio.play_audio(&sounds[0]);
        }
        println!(
            "{}",
            format!("New objective: {}", self.objective).color(Color::BrightYellow)
        );
    }

    // sets the next objective for the robot
    fn set_next(&mut self, objective: Objective) {
        self.next = objective
    }

    // replaces the current objective with the next one
    fn next_objective(&mut self) {
        self.set_objective(self.next.clone());
        self.next = Objective::None;
    }

    // collects the tent from the map
    fn retrieve_tent(&mut self, world: &mut World) {
        if let Some(direction) = self.face_target(
            world,
            true,
            |tile| if let Tent(_) = tile.content { true } else { false },
        ) {
            println!("Tent is {direction:?}");
            if let Ok(_) = destroy(self, world, direction) {
                println!("Tent retrieved");
            }
        }
    }

    // tries to place the tent as close to the robot as it can;
    // in most cases it should be able to do so in one of the tiles closest to it
    fn place_tent(&mut self, world: &mut World) -> Result<(), ()> {
        // current amount of tents in inventory
        let current = *self.get_backpack().get_contents().get(&Tent(0)).unwrap_or(&0usize);

        // if there is no tent in the inventory, it tries to craft one
        if current == 0 {
            if craft(self, Tent(0)).is_ok() {
                println!("New tent crafted");
            } else {
                println!("Need materials to craft a new tent");
                println!("I'll just sleep here for today");
                return Ok(());
            }
        }

        let direction = self.face_target(world, true, |tile| {
            tile.content == Content::None
                && tile.tile_type.properties().can_hold(&Tent(0))
                && tile.tile_type.properties().walk()
        });

        println!("Looking for a place to put the tent.. ");
        // tries to place the tent in the obtained direction
        return if let Some(direction) = direction.as_ref() {
            println!("Placing tent {direction:?}");
            match put(self, world, Tent(0), 1, direction.clone()) {
                | Ok(_) => {
                    // if it managed to place the tent, it goes inside
                    let _ = go(self, world, direction.clone());
                    Ok(())
                }
                | Err(e) => {
                    eprintln!("{e:?}");
                    Err(())
                }
            }
        }

        // finds a suitable location to place the tent otherwise
        else {
            match Spyglass::new(
                self.get_coordinate().get_row(),
                self.get_coordinate().get_row(),
                3,
                robot_map(world).unwrap().len(),
                None,
                false,
                0.5,
                |tile| {
                    tile.content == Content::None
                        && tile.tile_type.properties().can_hold(&Tent(0))
                        && tile.tile_type.properties().walk()
                },
            )
                .new_discover(self, world)
            {
                | SpyglassResult::Stopped(vec) => {

                    // for some reason the spyglass will sometimes include tiles that do not match the criteria
                    for (tile, row, col) in vec.iter() {
                        if tile.content == Content::None
                            && tile.tile_type.properties().walk()
                            && tile.tile_type.properties().can_hold(&Tent(0)) {
                            println!("{}", format!("Found a place to sleep at ({},{})", row, col).color(Color::BrightGreen));
                            self.compass
                                .set_destination(Destination::Coordinate((*row, *col)));
                            self.set_next(Sleeping);
                            self.set_objective(Moving(false));
                            return Err(());
                        }
                    }
                    Err(())
                }
                | _ => {
                    println!("I'll just sleep here for today");
                    Ok(())
                }
            }
        };
    }

    // sets a destination in the zone that is recognized as least explored
    fn set_random_destination(&mut self, world: &mut World) {
        let mut map = robot_map(world).unwrap();
        let mut dim = map.len();

        // decide the precision (iterations of the loop) to apply to the search function
        // based on the map length and chance. This way, calling it more than once consecutively is not
        // guaranteed to yield the same result, and it might lead the robot to different areas even though they all
        // fall under 'least explored'
        let precision = random::<u32>() % dim.ilog2() + 1;
        println!("precision: {precision}");

        // setup to find the least explored
        let mut target = (dim / 2, dim / 2);
        let mut min = u32::MAX;
        let mut min_quadrant = 0;

        for _ in 1..=precision {
            // setup 4 quadrants from the map
            //       |
            //    0  |  1
            // ------|------
            //    3  |  2
            //       |
            let mut quadrant = Vec::new();
            for i in 0..=3 {
                quadrant.push(Vec::new());
                for row in 0..=dim / 2 {
                    quadrant[i].push(Vec::new());
                    for _col in 0..=dim / 2 {
                        // initially empty
                        quadrant[i][row].push(None);
                    }
                }
            }

            // fill the quadrants by mapping each tile from the robot map to the correct quadrant
            let (mut row, mut col) = (0usize, 0usize);
            while row < dim {
                while col < dim {
                    if row < dim / 2 {
                        if col < dim / 2 {
                            quadrant[0][row][col] = map[row][col].clone();
                        } else if col < dim {
                            quadrant[1][row][col - (dim / 2)] = map[row][col].clone();
                        }
                    } else if row < dim {
                        if col < dim / 2 {
                            quadrant[2][row - (dim / 2)][col] = map[row][col].clone();
                        } else if col < dim {
                            quadrant[3][row - (dim / 2)][col - (dim / 2)] = map[row][col].clone();
                        }
                    }
                    col += 1;
                }
                row += 1;
                col = 0;
            }

            // obtain the least explored quadrant by summing each entry in each quadrant, mapped as follows:
            // Some(Tile) => 1
            // None => 0
            for i in 0..=3 {
                let mut sum = 0;
                for row in quadrant[i].iter() {
                    for tile in row.iter() {
                        if tile.is_some() {
                            sum += 1;
                        }
                    }
                }
                // and comparing the sums of the tiles of each quadrant to find the minimum
                if sum < min {
                    min = sum;
                    min_quadrant = i;
                }
            }
            println!("Quadrant {min_quadrant} has {min} undiscovered tiles");
            // finally set as target coordinate the coordinate in the center of the quadrant
            target.0 = match min_quadrant {
                | 0 | 1 => target.0 - dim / 4,
                | 3 | 2 => target.0 + dim / 4,
                | _ => target.0,
            };
            target.1 = match min_quadrant {
                | 0 | 3 => target.1 - dim / 4,
                | 1 | 2 => target.1 + dim / 4,
                | _ => target.1,
            };

            // set the map to the previous minimum quadrant, for the eventual next loop
            dim /= 2;
            map = quadrant[min_quadrant].clone();
        }

        println!("Random destination set: {:?}", target);
        self.compass.set_destination(Destination::Coordinate(target));
        self.set_objective(Moving(false));
    }

    // tries to set the best destination given a target content and the next day's weather
    fn set_best_destination(
        &mut self,
        world: &mut World,
        target_content: Content,
        next_weather: Result<WeatherType, WeatherToolError>,
        discover_new: bool,
    ) {
        let mut destination_found = false;
        // if the weather is good, find the most loaded location
        // (assume it might be further away)
        if let Ok(WeatherType::Sunny) = next_weather {
            if let Ok(c) = self.map.find_most_loaded(world, self, target_content.clone()) {
                self.compass.set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                println!("Found the most {target_content} at {:?} in the map", swap_coordinates(c.into()));
                destination_found = true;
            }
        }
        // otherwise stick to the closest location, so that the bot doesn't go too far off the presumed safe spot it's in
        else if let Ok(c) = self.map.find_closest(world, self, target_content.clone()) {
            if !self.bankrupt.contains(&swap_coordinates(c.into())) {
                self.compass.set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                println!("{}", format!("Found the closest {target_content} at {:?} in the map", swap_coordinates(c.into())).color(Color::BrightGreen));
                destination_found = true;
            } else if let Ok(c) = self.map.find_most_loaded(world, self, target_content.clone()) {
                self.compass.set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                println!("Found the most {target_content} at {:?} in the map", swap_coordinates(c.into()));
                destination_found = true;
            }
        }

        if !destination_found {
            println!("{}", format!("{target_content} not found in the map").color(Color::BrightRed));
            self.set_objective(Exploring);
        } else {
            self.set_objective(Moving(discover_new));
        }
    }

    // if a target is present in the 9x9 around the robot, it makes sure the robot is actually facing it
    // and returns the correct direction
    fn face_target<Target: Fn(&Tile) -> bool>(
        &mut self,
        world: &mut World,
        move_allowed: bool,
        target: Target,
    ) -> Option<Direction> {
        for (i, row) in robot_view(self, world).iter().enumerate() {
            for (j, tile) in row.iter().enumerate() {
                if let Some(tile) = tile {
                    if target(tile) {
                        match (i, j) {
                            | (0, 1) => return Some(Up),
                            | (1, 0) => return Some(Left),
                            | (1, 2) => return Some(Right),
                            | (2, 1) => return Some(Down),
                            | (0, 0) => {
                                if move_allowed {
                                    if go(self, world, Left).is_ok() {
                                        return Some(Up);
                                    } else if go(self, world, Up).is_ok() {
                                        return Some(Left);
                                    }
                                }
                            }
                            | (0, 2) => {
                                if move_allowed {
                                    if go(self, world, Right).is_ok() {
                                        return Some(Up);
                                    } else if go(self, world, Up).is_ok() {
                                        return Some(Right);
                                    }
                                }
                            }
                            | (1, 1) => {
                                if move_allowed {
                                    if go(self, world, Down).is_ok() {
                                        return Some(Up);
                                    } else if go(self, world, Up).is_ok() {
                                        return Some(Down);
                                    } else if go(self, world, Left).is_ok() {
                                        return Some(Right);
                                    } else if go(self, world, Right).is_ok() {
                                        return Some(Left);
                                    }
                                }
                            }
                            | (2, 0) => {
                                if move_allowed {
                                    if go(self, world, Left).is_ok() {
                                        return Some(Down);
                                    } else if go(self, world, Down).is_ok() {
                                        return Some(Left);
                                    }
                                }
                            }
                            | (2, 2) => {
                                if move_allowed {
                                    if go(self, world, Right).is_ok() {
                                        return Some(Down);
                                    } else if go(self, world, Down).is_ok() {
                                        return Some(Right);
                                    }
                                }
                            }
                            | _ => {}
                        }
                    }
                }
            }
        }
        return None;
    }

    // returns the coordinates of the next tile in the direction provided
    fn look_ahead(&self, world: &World, direction: Direction) -> Option<(usize, usize)> {
        let (row, col) = self.get_coordinate_usize();
        let dim = robot_map(world).unwrap().len();
        match direction {
            | Up => if row > 0 { Some((row - 1, col)) } else { None },
            | Down => if row < dim - 1 { Some((row + 1, col)) } else { None },
            | Left => if col > 0 { Some((row, col - 1)) } else { None },
            | Right => if col < dim - 1 { Some((row, col + 1)) } else { None },
        }
    }

    // blindly walks towards the current destination
    // called when path finding fails. If it looks like this and NLA are always fighting to
    // bring the robot to what they think is the right direction, that's because they are
    fn move_blindly(&mut self, world: &mut World) {
        // notify via audio or terminal of the takeover
        if let (Some(audio), Some(sounds)) = (self.audio.as_mut(), self.sounds.as_ref()) {
            let _ = audio.play_audio(&sounds[1]);
        } else {
            println!("{}", "Following my heart and not my compass".color(Color::BrightRed));
        }

        if let Some(Destination::Coordinate((dest_row, dest_col))) = *self.compass.get_destination() {

            // try to take between 2 and 8 steps  going blindly towards the destination
            // (it will stop if it's stuck)
            let steps = random::<u8>() % 4 + 1;
            let (mut stuck_row, mut stuck_col) = (false, false);
            for _ in 1..=steps {
                let (curr_row, curr_col) = self.get_coordinate_usize();
                if let Err(MoveError::AlreadyAtDestination) = self.compass.get_move(&robot_map(world).unwrap(), self.get_coordinate_usize()) {
                    break;
                }
                if !stuck_row {
                    if curr_row < dest_row {
                        if go(self, world, Down).is_err() {
                            stuck_row = true;
                        }
                    } else if curr_row > dest_row {
                        if go(self, world, Up).is_err() {
                            stuck_row = true;
                        }
                    }
                }

                if let Err(MoveError::AlreadyAtDestination) = self.compass.get_move(&robot_map(world).unwrap(), self.get_coordinate_usize()) {
                    break;
                }
                if !stuck_col {
                    if curr_col < dest_col {
                        if go(self, world, Right).is_err() {
                            stuck_col = true;
                        }
                    } else if curr_col > dest_col {
                        if go(self, world, Left).is_err() {
                            stuck_col = true;
                        }
                    }
                }
                if stuck_row && stuck_col {
                    break;
                } else {
                    self.gui.as_mut().map(|gui| { gui.update_world(robot_map(world).unwrap()); });
                }
            }

            // refresh the destination
            if self.compass.get_destination().is_some() {
                self.compass.clear_destination();
                self.compass
                    .set_destination(Destination::Coordinate((dest_row, dest_col)));
            }
        }
        // it should never get in here as I always use Destination::Coordinate when moving,
        // but just in case I am wrong set a random destination
        else {
            self.set_random_destination(world);
        }

        // remove the last two coordinates from the list, then add back the current ones
        // this way the robot is allowed to go back on its steps once,
        // in case move_towards got it stuck
        self.last_coords.pop();
        self.last_coords.pop();
        self.last_coords.push(self.get_coordinate_usize());
    }

    // returns the best content to sell at the moment, based on quantity
    // owned and price
    fn get_content_to_sell(&self) -> Content {
        let (mut max_content, mut max_cost) = (Content::None, 0usize);

        // costs of the various items at the shop
        let mut costs = HashMap::new();
        costs.insert(Content::Rock(0), 1);
        costs.insert(Content::Tree(0), 2);
        costs.insert(Content::Fish(0), 3);
        costs.insert(Content::Coin(0), 3);

        for (content, quantity) in self.get_backpack().get_contents().iter() {
            if let Content::Tree(_) | Content::Rock(_) | Content::Fish(_) = content {
                let cost = *quantity * *costs.get(&content).unwrap();
                if cost > max_cost {
                    max_cost = cost;
                    max_content = content.clone();
                }
            }
        }
        max_content
    }

    // main function, called each tick
    fn auto_pilot(&mut self, world: &mut World, assisted: bool) {
        if let DayTime::Night = look_at_sky(world).get_time_of_day() {
            if let Sleeping | Waiting(_) = self.objective {
                // the robot is already sleeping
            } else if let Sleeping = self.next {
                // the robot is moving towards a place to put the tent
            } else {
                println!("{}", "-> time to sleep!".color(Color::BrightMagenta));
                self.set_objective(Sleeping);
            }
        }

        // check if energy level critical
        else if self.get_energy().get_energy_level() < 150 {
            match (&self.objective, &self.next) {
                | (&Waiting(_), _) | (&Charging(_), _) | (&Sleeping, _) | (_, &Sleeping) => {}
                | _ => {
                    self.set_next(self.objective.clone());
                    self.set_objective(Charging(250));
                }
            }
        }

        match self.objective.clone() {
            // the robot is deciding what to do next
            // either on autopilot or by user choice
            | Praying => {

                // remove any destination, which is no longer relevant
                self.compass.clear_destination();
                let mut pilot_objective = Objective::None;

                if assisted {
                    println!("Backpack :{:?}", self.get_backpack()
                        .get_contents()
                        .iter()
                        .filter(|(_, val)| **val > 0)
                        .collect::<HashMap<&Content, &usize>>());

                    // get the objective chosen by the pilot
                    if let Some(pilot) = self.pilot.as_mut() {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        println!("{}", "Decide what to do now:".color(Color::BrightWhite));
                        pilot_objective = match pilot.get_objective() {
                            | Ok(o) => o,
                            | Err(_) => {
                                self.pilot = None;
                                Objective::None
                            }
                        }
                    }
                }

                if assisted && pilot_objective != Objective::None {
                    // do what the pilot decided
                    self.set_next(Objective::None);
                    self.set_objective(pilot_objective);
                }

                // if there is no pilot, or they decided not to intervene,
                // go on autopilot and let the AI decide what to do
                else {
                    let weather = look_at_sky(world).get_weather_condition();
                    let next_weather = self.forecast.predict_from_time(0, 24);

                    // if current weather is bad, sleep for the day
                    if let WeatherType::TrentinoSnow | WeatherType::TropicalMonsoon = weather {
                        println!("The weather today is {weather:?}");
                        self.set_objective(Sleeping);
                    }

                    // if the weather for the next day is bad, move close to a town
                    else if let Ok(WeatherType::TrentinoSnow | WeatherType::TropicalMonsoon) = next_weather.as_ref() {
                        println!("The weather tomorrow is {:?}", next_weather.as_ref().unwrap());
                        print!("Decided to reach shelter from tomorrow's storm and ");
                        if let Ok(c) = self.map.find_closest(world, self, Content::Building) {
                            println!("found some buildings");
                            self.set_objective(Moving(true));
                            self.compass.set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                        } else if let Ok(c) = self.map.find_closest(world, self, Content::Market(0)) {
                            println!("found a market");
                            self.set_objective(Moving(true));
                            self.compass.set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                        } else if let Ok(c) = self.map.find_closest(world, self, Content::Bank(0..0)) {
                            println!("found a bank");
                            self.set_objective(Moving(true));
                            self.compass.set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                        } else if let Ok(c) = self.map.find_closest(world, self, Content::Tree(0)) {
                            println!("found a tree");
                            self.compass.set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                            self.set_objective(Moving(false));
                        } else {
                            println!("found none! to exploring then");
                            self.set_objective(Exploring)
                        }

                        // wait till the night at the shelter
                        self.set_next(Waiting(DayTime::Night));
                    }

                    // if the backpack is more than 80% full, go to the market and sell
                    else if self.get_backpack().get_contents().values().sum::<usize>()
                        >= self.get_backpack().get_size() * 4 / 5 {

                        // select the item that would make the most money in the current held quantity
                        let sellable_content = self.get_content_to_sell();

                        // check if it-s better to deposit coins in the bank before
                        let backpack = self.get_backpack().get_contents();
                        let target_content = if *backpack.get(&Content::Coin(0)).unwrap_or(&0)
                            > *backpack.get(&sellable_content).unwrap_or(&0)
                        {
                            println!("Decided to deposit my coins");
                            self.set_next(Depositing);
                            Content::Bank(0..0)
                        } else {
                            println!("Decided to sell some {sellable_content}");
                            self.set_next(Selling(sellable_content.clone()));
                            Content::Market(0)
                        };
                        self.set_best_destination(world, target_content.clone(), next_weather, true);
                    }

                    // if the backpack is less than 60% full, gather some content
                    else if self.get_backpack().get_contents().values().sum::<usize>()
                        <= self.get_backpack().get_size() * 3 / 5 {

                        // select the item of which the robot holds most
                        let (mut max_content, mut max_quantity) = (Vec::new(), 0);
                        for (content, quantity) in self.get_backpack().get_contents().iter() {
                            if let Content::Tree(_) | Content::Rock(_) | Content::Fish(_) = content {
                                if *quantity > max_quantity {
                                    max_quantity = *quantity;
                                    max_content.clear();
                                    max_content.push(content.clone());
                                } else if *quantity == max_quantity {
                                    max_content.push(content.clone());
                                }
                            }
                        }

                        // choose randomly if more than one have the same quantity
                        let range = max_content.len();
                        let target_content = max_content[random::<usize>() % range].clone();


                        println!("Decided to gather some {target_content}");
                        self.set_next(Gathering(target_content.clone()));
                        self.set_best_destination(world, target_content.clone(), next_weather, false);
                    }

                    // if there is nothing else to do, explore
                    else {
                        println!("Decided to explore");
                        self.set_objective(Exploring);
                    }
                }
            }

            | Waiting(target_time) => {
                println!(".");
                std::thread::sleep(std::time::Duration::from_millis(100));

                let current_time = look_at_sky(world).get_time_of_day();
                if current_time == target_time {
                    if let DayTime::Morning = current_time {
                        println!("{}", "-> time to wake up!".color(Color::BrightYellow));
                    } else {
                        println!("{}", "-> finished waiting".color(Color::BrightCyan));
                    }
                    // retrieves the tent if it's not already in the inventory
                    if *self.get_backpack().get_contents().get(&Tent(0)).unwrap() == 0usize {
                        self.retrieve_tent(world);
                    }
                    self.next_objective();
                }
            }

            // the robot is moving to some destination
            // the next objective is stored inside the enum
            | Moving(discover_new) => {
                let map = robot_map(world);

                // 25% chance to pick up random content while moving around
                if let Some(direction) = self.face_target(world, false, |tile| {
                    if let Content::Rock(_) | Content::Tree(_) | Content::Fish(_) | Content::Coin(_) = tile.content {
                        true
                    } else {
                        false
                    }
                }) {
                    let backpack = self.get_backpack();
                    if random::<usize>() % 4 == 0
                        && backpack.get_contents().values().sum::<usize>() < backpack.get_size()
                    {
                        if let Ok(_) = destroy(self, world, direction) {
                            println!("Picked up some supplies while moving");
                        }
                    }
                }

                // 10% chance to use the spyglass with a reduced range, also to help with NLA pathfinding
                if random::<u8>() % 10 == 0 {
                    let _ = Spyglass::new(
                        self.get_coordinate().get_row(),
                        self.get_coordinate().get_col(),
                        5,
                        robot_map(world).unwrap().len(),
                        Some(self.get_energy().get_energy_level() / 5),
                        false,
                        0.5,
                        |_| false,
                    )
                        .new_discover(self, world);
                }

                // need to constantly take random turns due to a bug in NLA compass,
                // sometimes the robot just goes back and forth between two adjacent tiles
                // this is not a fix, it still does that sometimes, but it seems to give
                // the compass the opportunity to fix itself in some occasions
                match self
                    .compass
                    .get_move(&map.clone().unwrap(), self.get_coordinate_usize())
                {
                    | Ok(direction) => {
                        // if the robot keeps going back on its steps, leave a chance to
                        // intervene and manually move it towards the destination, as it might mean it's stuck
                        if let Some(next) = self.look_ahead(world, direction.clone()) {
                            if self.last_coords.contains(&next) && random::<u8>() % 2 == 0
                            {
                                self.move_blindly(world);
                            }
                        }

                        if let Err(LibError::CannotWalk) = go(self, world, direction.clone()) {
                            println!("Can't go {direction:?} from here");

                            // if the robot is moving towards a content
                            if let Gathering(content) = self.next.clone() {
                                println!("Next I have to be {:?}", self.next);

                                // checks if the content is actually right in front of it
                                // (mostly happens in case of fish)
                                if let Some(direction) = self
                                    .face_target(world, true, |tile| tile.content.to_default() == content.to_default())
                                {
                                    println!("{content} is reachable {direction:?} from here");
                                    let _ = destroy(self, world, direction);
                                    self.compass.clear_destination();

                                    if discover_new {
                                        self.pins.insert(self.get_coordinate_usize());
                                    }
                                    self.next_objective();
                                } else {
                                    // otherwise it needs to build a bridge to it
                                    print!("Building a road to the {content}..");
                                    let mut i = 1;
                                    loop {
                                        print!(".");

                                        // try to put as few rocks as I can to build the bridge
                                        match put(self, world, Content::Rock(i), i, direction.clone()) {
                                            // if there is some content already, try to destroy it
                                            | Err(LibError::MustDestroyContentFirst) => {
                                                if let Err(_) = destroy(self, world, direction.clone()) {
                                                    println!("can't reach that {content} right now..");
                                                    self.compass.clear_destination();
                                                    self.set_objective(Praying);
                                                    break;
                                                }
                                            }

                                            // if the rocks provided weren't enough, try with one more
                                            // (as long as the inventory has them)
                                            | Err(LibError::NotEnoughContentProvided) => {
                                                if *self.get_backpack().get_contents().get(&Content::Rock(0)).unwrap()
                                                    < i
                                                {
                                                    println!("\nI don't have enough rocks right now..");
                                                    self.compass.clear_destination();
                                                    self.set_next(Gathering(Content::Rock(0)));
                                                    self.set_objective(Exploring);
                                                    break;
                                                } else {
                                                    i += 1;
                                                }
                                            }

                                            // charge if not enough energy
                                            | Err(LibError::NotEnoughEnergy) => {
                                                println!(
                                                    "\nI'm too low on energy ({}/1000)",
                                                    self.get_energy().get_energy_level()
                                                );
                                                self.set_objective(Charging(300));
                                                break;
                                            }

                                            | Ok(_) => {
                                                println!(" done!");
                                                break;
                                            }
                                            | _ => {
                                                println!("can't reach that {content} right now..");
                                                self.compass.clear_destination();
                                                self.set_next(Objective::None);
                                                self.set_objective(Praying);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }

                            // if the robot is stuck, make it go back on its steps
                            else {
                                let oldest_saved_position = self.last_coords.first();
                                let current_position = self.get_coordinate_usize();
                                if let Some(coordinate) = oldest_saved_position {
                                    if let (Some(audio), Some(sounds)) = (self.audio.as_mut(), self.sounds.as_ref()) {
                                        let _ = audio.play_audio(&sounds[1]);
                                    } else { println!("{}", "I hit an obstacle, backtracking".color(Color::BrightRed)); }
                                    self.compass.set_destination(Destination::Coordinate(*coordinate));
                                    self.last_coords.clear();
                                    self.last_coords.push(current_position);
                                }
                            }
                        } else {
                            self.gui.as_mut().map(|gui| {
                                gui.update_world(robot_map(world).unwrap());
                            });
                        }
                    }
                    | Err(e) => {
                        // abort objective in case of error in the compass
                        let msg: &str;
                        match e {
                            | MoveError::NoDestination => msg = "not set",
                            | MoveError::NoContent => msg = "unavailable (no content)",
                            | MoveError::NoTileType => msg = "unavailable (no tile type)",
                            | MoveError::InvalidCurrPosition => msg = "unreachable (invalid start)",
                            | MoveError::InvalidDestCoordinate => msg = "unreachable (invalid end)",
                            | MoveError::NoAvailableMove => msg = "unreachable (no move)",
                            | MoveError::AlreadyAtDestination => {
                                msg = "reached";
                            }
                            | MoveError::NotImplemented => msg = "what the f!#@??",
                        }
                        println!("Destination is {msg}");
                        self.next_objective();

                        // if the destination needs to be added to the pins, do so
                        // this is needed because when randomly exploring the spyglass
                        // is set to look for buildings, markets and banks first
                        // and if I let it go to any one of those it finds, there is a large
                        // possibility that it will just go back to the same building every time
                        if let MoveError::AlreadyAtDestination = e {
                            if discover_new {
                                self.pins.insert(self.get_coordinate_usize());
                            }
                        }
                    }
                }
            }

            // the robot needs to charge up to a certain level
            | Charging(target_level) => {
                println!(
                    "{}",
                    format!("Charge: {}/{target_level}", self.get_energy().get_energy_level()).color(Color::Cyan)
                );
                // if the desired charge level is reached,
                // decide what to do next
                if self.get_energy().get_energy_level() >= target_level {
                    self.next_objective();
                }
                // else pass
            }

            // the robot needs to find a place to sleep,
            // then stays put till morning
            | Sleeping => {
                if self.place_tent(world).is_ok() {
                    self.set_next(Praying);
                    self.set_objective(Waiting(
                        // if for some reason the robot decides to sleep in the morning, wait till night first and then repeat
                        if let DayTime::Morning = look_at_sky(world).get_time_of_day() {
                            DayTime::Night
                        } else {
                            DayTime::Morning
                        },
                    ));
                } else {
                    println!("{}", "Couldn't place the tent".color(Color::BrightRed))
                }
            }

            // the robot needs to gather some type of content
            | Gathering(content) => {
                // at the time of writing this
                // collect all seems not to collect the content you're directly standing on,
                // and since the NLA compass takes you exactly on it, I need to move to face
                // the content and only then use collect all
                if let Some(direction) =
                    self.face_target(world, true, |tile| tile.content.to_default() == content.to_default()) {
                    let _ = destroy(self, world, direction);
                }

                // collect more content in the area if there is enough space in the backpack
                // also do not do it with fish because it will bug out greatly
                if content.to_default() != Content::Fish(0)
                    && self.get_backpack().get_contents().values().sum::<usize>() < self.get_backpack().get_size() * 4 / 5 {
                    let mut requirements = HashMap::new();
                    let space_left =
                        self.get_backpack().get_size() - self.get_backpack().get_contents().values().sum::<usize>();
                    requirements.insert(content.to_default(), space_left);
                    println!("Collecting all {content} in the area");
                    CollectAll::collect_items(self, world, 3, requirements);
                }

                let next_weather = self.forecast.predict_from_time(0, 24).unwrap_or(WeatherType::Sunny);

                // if there is still space in the backpack and there is no storm incoming, continue the gathering streak
                if self.get_backpack().get_contents().values().sum::<usize>() < self.get_backpack().get_size() * 4 / 5 {
                    if assisted || (next_weather != WeatherType::TrentinoSnow && next_weather != WeatherType::TropicalMonsoon) {
                        // set the status to MOVING for the next ticks,
                        // in order to move to the next closest area with the target content
                        if let Ok(c) = self.map.find_closest(world, self, content.clone()) {
                            let c = swap_coordinates(c.into());
                            println!("Found {} at ({}, {})", content, c.0, c.1);
                            self.compass.set_destination(Destination::Coordinate(c));
                            self.set_objective(Moving(false));
                        } else {
                            println!("No {} found in the vicinity, need to explore", content);
                            self.set_objective(Exploring);
                        }
                        self.set_next(Gathering(content));
                    }
                }
                // otherwise go selling
                else {
                    println!("{}", "Backpack too full, selling".color(Color::BrightRed));
                    let sellable_content = self.get_content_to_sell();
                    self.set_objective(Selling(sellable_content));
                    self.set_next(Objective::None);
                }
            }

            // the robot needs to sell some type of content
            | Selling(content) => {
                let quantity_held = *self.get_backpack().get_contents().get(&content).unwrap_or(&0);
                let mut transaction_ok = false;

                // look for market in the vicinity in which to sell the content
                match self.face_target(world, true,
                                       |tile| { if let Content::Market(n) = tile.content { n >= quantity_held } else { false } }) {
                    // if there is no un-depleted market nearby
                    | None => {
                        // the map might still tag as most loaded the depleted market in front of the robot, since it might be the only market discovered
                        if let Ok(c) = self.map.find_most_loaded(world, self, Content::Market(0)) {

                            // if it's not the case, move towards the newfound market
                            if !self.bankrupt.contains(&swap_coordinates(c.into())) {
                                println!("Market found at {:?}", swap_coordinates(c.into()));
                                self.compass.set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                                transaction_ok = true;
                                self.set_next(Selling(content.clone()));
                                self.set_objective(Moving(true));
                            }
                        }
                    }
                    // if the market is close to the robot
                    | Some(direction) => {
                        match put(self, world, content.clone(), quantity_held, direction.clone()) {
                            | Ok(quantity_sold) => {
                                // if the robot sold all the content in the backpack
                                if quantity_sold == quantity_held {
                                    if let (Some(audio), Some(sounds)) = (self.audio.as_mut(), self.sounds.as_ref()) {
                                        let _ = audio.play_audio(&sounds[2]);
                                    }
                                    transaction_ok = true;
                                    self.next_objective();
                                }
                                // otherwise find another market
                                else {
                                    if let Some(c) = self.look_ahead(world, direction.clone()) {
                                        self.bankrupt.insert(c);
                                    }
                                    if let Ok(c) = self.map.find_most_loaded(world, self, Content::Market(0)) {
                                        // and check again that it's not the same one
                                        if self.look_ahead(world, direction).unwrap_or((0, 0)) != swap_coordinates(c.into()) {
                                            println!("Market depleted, new one found at {:?}", swap_coordinates(c.into()));
                                            transaction_ok = true;
                                            self.compass.set_destination(Destination::Coordinate(c.into()));
                                            self.set_next(Selling(content.clone()));
                                            self.set_objective(Moving(true));
                                        }
                                    }
                                }
                            }
                            // if there wasn't enough space for all the coins, deposit some
                            | Err(LibError::NotEnoughSpace(_)) => {
                                println!("I'm too rich, better deposit some Coins");
                                if let (Some(audio), Some(sounds)) = (self.audio.as_mut(), self.sounds.as_ref()) {
                                    let _ = audio.play_audio(&sounds[2]);
                                }
                                transaction_ok = true;
                                self.set_objective(Depositing);
                            }
                            | Err(e) => {
                                eprintln!("{e:?}");
                            }
                        }
                    }
                }
                if !transaction_ok {
                    println!("{}", "No market found".color(Color::BrightRed));
                    self.compass.clear_destination();
                    self.set_objective(Exploring);
                }
            }

            | Depositing => {
                let quantity_held = *self.get_backpack().get_contents().get(&Content::Coin(0)).unwrap_or(&0);
                let mut transaction_ok = false;

                // look for bank
                match self.face_target(world, true,
                                       |tile| {
                                           if let Content::Bank(range) = &tile.content { range.clone().sum::<usize>() > 0 } else { false }
                                       }) {
                    | None => {
                        if let Ok(c) = self.map.find_most_loaded(world, self, Content::Bank(0..0)) {
                            if !self.bankrupt.contains(&swap_coordinates(c.into())) {
                                println!("Bank found at {:?}", swap_coordinates(c.into()));
                                self.compass.set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                                transaction_ok = true;
                                self.set_next(Depositing);
                                self.set_objective(Moving(true));
                            }
                        }
                    }
                    // if the bank is close to the robot
                    | Some(direction) => {
                        match put(self, world, Content::Coin(0), quantity_held, direction.clone()) {
                            | Ok(quantity_deposited) => {
                                if quantity_deposited == quantity_held {
                                    if let (Some(audio), Some(sounds)) = (self.audio.as_mut(), self.sounds.as_ref()) {
                                        let _ = audio.play_audio(&sounds[2]);
                                    }
                                    transaction_ok = true;
                                    self.next_objective();
                                } else {
                                    // the bank is depleted
                                    if let Some(c) = self.look_ahead(world, direction) {
                                        self.bankrupt.insert(c);
                                    }
                                    if let Ok(c) = self.map.find_most_loaded(world, self, Content::Market(0)) {
                                        if !self.bankrupt.contains(&swap_coordinates(c.into())) {
                                            println!("Bank depleted, new one found at {:?}", swap_coordinates(c.into()));
                                            transaction_ok = true;
                                            self.compass.set_destination(Destination::Coordinate(c.into()));
                                            self.set_next(Depositing);
                                            self.set_objective(Moving(true));
                                        }
                                    }
                                }
                            }
                            | Err(LibError::NotEnoughEnergy) => {
                                self.set_next(Depositing);
                                self.set_objective(Charging(self.get_energy().get_energy_level() + 100));
                            }
                            | Err(e) => {
                                eprintln!("{e:?}");
                            }
                        }
                    }
                }
                if !transaction_ok {
                    println!("{}", "No Bank found".color(Color::BrightRed));
                    self.compass.clear_destination();
                    self.set_objective(Exploring);
                }
            }

            | Exploring => {
                let dim = robot_map(world).unwrap().len();
                let mut mark_visited = false;
                let mut target_content = Vec::new();

                // there probably was a better way to do this but
                // doing something like
                // if let Gathering(content_to_gather) = self.next.clone() {
                //      let stops_when = |tile| {
                //            tile.content.to_default() = content_to_gather.to_default()
                //      }
                // }
                // isn't possible
                let mut stops_when: fn(&Tile) -> bool = |_| false;
                if let Gathering(content) = &self.next {
                    match content {
                        | Content::Rock(_) => {
                            stops_when = |tile| {
                                if let Content::Rock(_) = tile.content {
                                    true
                                } else {
                                    false
                                }
                            }
                        }
                        | Content::Tree(_) => {
                            stops_when = |tile| {
                                if let Content::Tree(_) = tile.content {
                                    true
                                } else {
                                    false
                                }
                            }
                        }
                        | Content::Fish(_) => {
                            stops_when = |tile| {
                                if let Content::Fish(_) = tile.content {
                                    true
                                } else {
                                    false
                                }
                            }
                        }
                        | _ => {}
                    }
                    target_content.push(content.clone());
                } else {
                    mark_visited = true;
                    target_content.push(Content::Bank(0..0));
                    target_content.push(Content::Market(0));
                    target_content.push(Content::Building);
                    stops_when = |tile| match tile.content {
                        | Content::Bank(_) | Content::Market(_) | Content::Building => true,
                        | _ => false,
                    };
                }

                let mut spyglass = Spyglass::new(
                    self.get_coordinate().get_row(),
                    self.get_coordinate().get_col(),
                    dim / 2,
                    dim,
                    Some(self.get_energy().get_energy_level() / 2),
                    true,
                    0.5,
                    stops_when,
                );

                let mut destination_found = false;
                if let SpyglassResult::Stopped(vec) = spyglass.new_discover(self, world) {

                    // for some reason the spyglass will sometimes include tiles that do not match the criteria
                    for (tile, row, col) in vec.iter() {
                        if !mark_visited
                            || (mark_visited && !self.pins.contains(&(*row, *col)))
                            && target_content.contains(&tile.content.to_default())
                        {
                            println!(
                                "{}",
                                format!("Found {} at ({}, {}) with my spyglass", tile.content, row, col)
                                    .color(Color::BrightGreen)
                            );
                            destination_found = true;
                            self.compass.set_destination(Destination::Coordinate((*row, *col)));
                            self.set_objective(Moving(mark_visited));
                            break;
                        }
                    }
                }
                if !destination_found {
                    println!("Nothing found with the spyglass, setting random destination");
                    self.set_random_destination(world);
                }
            }

            | Objective::None => {
                let _ = robot_view(self, world);
                // if the map is more than 75% explored and there is nothing left to do, end the game
                if robot_map(world)
                    .unwrap()
                    .iter()
                    .map(|row| {
                        row.iter()
                            // map each tile to None = 0, Some = 1
                            .map(|option| if option.is_some() { 1 } else { 0 })
                            .sum::<usize>()
                    })
                    .sum::<usize>()
                    // check that the sum is > than 75% of the area of the map
                    > robot_map(world).unwrap().len().pow(2) * 3 / 4

                    // this one checks that there are no more active markets or banks
                    && Map::collection(&world)
                    .unwrap_or(HashMap::new())
                    .iter()
                    // filter the HashMap for Markets and Banks only
                    .filter(|(k, v)|
                        if **k == mem::discriminant(&Content::Market(0)) || **k == mem::discriminant(&Content::Bank(0..0)) {
                            // filter the vector of coordinates to find the ones that aren't in the bankrupt list
                            !v.iter()
                                .filter(|(c, _)|
                                    !self
                                        .bankrupt
                                        .contains(&swap_coordinates((*c).into())))
                                .collect::<Vec<_>>()
                                .is_empty()
                        } else { false })
                    .collect::<Vec<_>>()
                    // if there are either 0 more active markets or 0 more active banks, end the game
                    .len() < 2 {
                    println!("{}", "Congratulations, you beat the game!".color(Color::BrightYellow));
                    println!("Your score: {}", self.score);
                    self.handle_event(Event::Terminated);
                }
                // otherwise go on with any next objective
                else if let Objective::None = self.next {
                    self.set_objective(Praying);
                } else {
                    self.next_objective();
                }
            }
        }
    }

    fn manual_pilot(&mut self, world: &mut World) {
        println!(
            "{}",
            format!("Energy: {}/1000", self.get_energy().get_energy_level()).color(Color::BrightCyan)
        );

        if let Some(pilot) = self.pilot.as_mut() {
            match pilot.get_action() {
                9 /* go up */ =>
                    { let _ = go(self, world, Up); }

                8 /* go down */ =>
                    { let _ = go(self, world, Down); }
                7 /*go left */ =>
                    { let _ = go(self, world, Left); }

                6 /* go right */ =>
                    { let _ = go(self, world, Right); }

                5 /* destroy */ => {
                    if let Some(direction) = self.face_target(world, false,
                                                              |tile| tile.content.properties().destroy()) {
                        let _ = destroy(self, world, direction);
                    }
                }

                4 /* place tent */ => {
                    if let Some(direction) = self.face_target(world, false,
                                                              |tile| tile.tile_type.properties().can_hold(&Tent(0))) {
                        if *self.get_backpack().get_contents().get(&Tent(0)).unwrap_or(&0) == 0 {
                            let _ = craft(self, Tent(0));
                        }
                        let _ = put(self, world, Tent(0), 1, direction);
                    }
                }
                3 /* discover */ => {
                    let _ = Spyglass::new(
                        self.get_coordinate().get_row(),
                        self.get_coordinate().get_col(),
                        10,
                        robot_map(world).unwrap().len(),
                        None,
                        false,
                        0.5,
                        |_| false,
                    ).new_discover(self, world);
                }
                2 /* sell */ => {
                    let content = self.get_content_to_sell();
                    if let Some(direction) = self.face_target(world, false,
                                                              |tile| if let Content::Market(_) = tile.content { true } else { false })
                    {
                        let quantity = self.get_backpack().get_contents().get(&content).unwrap_or(&0usize);
                        let _ = put(self, world, content, *quantity, direction);
                    }
                }
                1 /* deposit */ => {
                    if let Some(direction) = self.face_target(world, false,
                                                              |tile| if let Content::Bank(_) = tile.content { true } else { false })
                    {
                        let quantity = self.get_backpack().get_contents().get(&Content::Coin(0)).unwrap_or(&0usize);
                        let _ = put(self, world, Content::Coin(0), *quantity, direction);
                    }
                }

                -1 => {
                    // some error occurred, remove pilot
                    self.pilot = None;
                }
                _ => {}
            }
        }
    }
}


impl Runnable for PioneerBot<'_> {
    fn process_tick(&mut self, world: &mut World) {
        // add some delay if the gui is not in use
        if self.gui.is_none() {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        // try to reconnect the pilot, in case the pico got unplugged
        if self.pilot.is_none() {
            self.pilot = Pilot::new().ok();
        }

        match self.pilot.as_ref() {
            | Some(pilot) => {
                // if the pilot is manual, get the input action
                if pilot.is_manual() {
                    self.manual_pilot(world);
                }
                // there is a pilot, but it's not manual: go assisted mode
                else {
                    self.auto_pilot(world, true);
                }
            }
            | None => {
                // there is no pilot: go full autopilot
                self.auto_pilot(world, false)
            }
        }

        // update the gui
        self.gui.as_mut().map(|gui| {
            gui.update_world(robot_map(world).unwrap());
        });

        // update the score
        self.score = get_score(world);

        // easter egg
        if random::<u32>() % 1000 == 0 {
            if let (Some(audio), Some(sounds)) = (self.audio.as_mut(), self.sounds.as_ref()) {
                let _ = audio.play_audio(&sounds[3]);
            }
        }
    }

    fn handle_event(&mut self, event: Event) {
        // play background music
        self.audio.as_mut().map(|audio| audio.play_audio_based_on_event(&event));
        match event {
            | Event::Ready => {
                let coords = self.get_coordinate_usize();
                if let Some(gui) = self.gui.as_mut() {
                    gui.add_robot(coords.1, coords.0);
                    gui.update_robot(Some(coords), Some(coords));
                }
                self.last_coords.push(coords);
            }
            | Event::Terminated => {
                *self.running.borrow_mut() = false;
            }
            | Event::TimeChanged(e) => {
                self.gui.as_mut().map(|gui| {
                    gui.update_weather(e.get_weather_condition());
                    gui.update_time_of_day(e.get_time_of_day());
                });
                self.forecast.process_event(&Event::TimeChanged(e));
            }
            | Event::DayChanged(_) => {
                println!("Score: {}", self.score);
                self.pilot.as_mut().map(|pilot| pilot.put_score(self.score));
            }
            | Event::EnergyRecharged(_) => {}
            | Event::EnergyConsumed(_) => {}
            | Event::Moved(_, coords) => {
                if self.last_coords.len() > 8 {
                    self.last_coords.remove(0);
                }
                let last = self.last_coords.last();
                if let Some(gui) = self.gui.as_mut() {
                    gui.update_robot(Some(coords), last.map(|c| *c));
                    match gui.tick() {
                        | Ok(_) => {}
                        | Err(_) => self.handle_event(Event::Terminated),
                    }
                }
                self.last_coords.push(coords);
                println!("-> Moved to {coords:?}");
                if let (Some(audio), Some(sounds)) = (self.audio.as_mut(), self.sounds.as_ref()) {
                    let _ = audio.play_audio(&sounds[4]);
                }
            }
            | Event::TileContentUpdated(_, _) => {}
            | Event::AddedToBackpack(content, quantity) => {
                println!(
                    "{}",
                    format!("-> {quantity} {content} added to backpack").color(Color::BrightCyan)
                );
            }
            | Event::RemovedFromBackpack(content, quantity) => {
                println!(
                    "{}",
                    format!("-> {quantity} {content} removed from backpack").color(Color::BrightMagenta)
                );
            }
        }
    }

    fn get_energy(&self) -> &Energy {
        &self.robot.energy
    }

    fn get_energy_mut(&mut self) -> &mut Energy {
        &mut self.robot.energy
    }

    fn get_coordinate(&self) -> &Coordinate {
        &self.robot.coordinate
    }

    fn get_coordinate_mut(&mut self) -> &mut Coordinate {
        &mut self.robot.coordinate
    }

    fn get_backpack(&self) -> &BackPack {
        &self.robot.backpack
    }

    fn get_backpack_mut(&mut self) -> &mut BackPack {
        &mut self.robot.backpack
    }
}

// I hid these functions down here because I'm ashamed of them

// swap the coordinates
// NLA compass, spyglass and robotics_lib all use (row, column) to index the map
// while tile/resource mapper does the opposite
fn swap_coordinates(c: (usize, usize)) -> (usize, usize) {
    // I literally cannot believe this needs to exist
    (c.1, c.0)
}

// converts the u8 readings from the serial port to
// the corresponding objective
impl From<u8> for Objective {
    fn from(value: u8) -> Self {
        match value {
            | 1 => Charging(750),
            | 2 => Selling(Content::Fish(0)),
            | 3 => Selling(Content::Tree(0)),
            | 4 => Selling(Content::Rock(0)),
            | 5 => Gathering(Content::Fish(0)),
            | 6 => Gathering(Content::Tree(0)),
            | 7 => Gathering(Content::Rock(0)),
            | 8 => Depositing,
            | 9 => Exploring,
            | _ => Objective::None,
        }
    }
}