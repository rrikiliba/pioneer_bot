use rand::random;
use robotics_lib::runner::Runnable;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::{Display, Formatter};
use std::rc::Rc;

use robotics_lib::energy::Energy;
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
use pmp_collect_all::CollectAll;
use rustbeef_nlacompass::compass::{Destination, MoveError, NLACompass as Compass};
use spyglass::spyglass::*;
use oxagaudiotool::{OxAgAudioTool};

use colored::{Color, Colorize};
use oxagaudiotool::sound_config::OxAgSoundConfig;
use crate::pilot::Pilot;
use crate::pioneer_bot::Objective::{
    Charging, Depositing, Exploring, Gathering, Moving, Praying, Selling, Sleeping, Waiting,
};
use robo_gui::MainState;
use robotics_lib::utils::LibError;
use robotics_lib::world::environmental_conditions::DayTime::Night;
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
    robot: Robot,
    pilot: Option<Pilot>,

    score: f32,
    objective: Objective,
    next: Objective,

    map: Map,
    pins: HashSet<(usize, usize)>,

    compass: Compass,
    forecast: Forecast,

    audio: Option<OxAgAudioTool>,
    gui: Option<MainState<'a>>,
    last_coords: Vec<(usize, usize)>,
    pub(crate) running: Rc<RefCell<bool>>,
}

impl PioneerBot<'_> {
    pub fn new(gui_start: bool, audio_start: bool) -> Self {
        Self {
            robot: Default::default(),
            pilot: Pilot::new().ok(),

            score: 0.,
            objective: Objective::None,
            next: Objective::None,

            map: Map {},
            pins: HashSet::new(),

            compass: Compass::new(),
            forecast: Forecast::new(),

            audio: if audio_start {
                let mut audio_map = HashMap::new();
                audio_map.insert(WeatherType::Sunny, OxAgSoundConfig::new_looped_with_volume("sounds\\WeatherType\\Sunny.mp3", 0.5));
                audio_map.insert(WeatherType::Rainy, OxAgSoundConfig::new_looped_with_volume("sounds\\WeatherType\\Rainy.mp3", 0.5));
                audio_map.insert(WeatherType::Foggy, OxAgSoundConfig::new_looped_with_volume("sounds\\WeatherType\\Foggy.mp3", 0.5));
                audio_map.insert(WeatherType::TrentinoSnow, OxAgSoundConfig::new_looped_with_volume("sounds\\WeatherType\\Snow.mp3", 0.5));
                audio_map.insert(WeatherType::TropicalMonsoon, OxAgSoundConfig::new_looped_with_volume("sounds\\WeatherType\\Monsoon.mp3", 0.5));

                let mut event_map = HashMap::new();
                event_map.insert(Event::RemovedFromBackpack(Content::None, 0), OxAgSoundConfig::new_with_volume("sounds\\Event\\Place.wav", 0.2));
                event_map.insert(Event::AddedToBackpack(Content::None, 0), OxAgSoundConfig::new_with_volume("sounds\\Event\\PickUp.wav", 0.2));

                OxAgAudioTool::new(event_map, Default::default(), audio_map).ok()
            } else { None },
            gui: if gui_start {
                Some(MainState::new(1).unwrap())
            } else { None },
            last_coords: Vec::new(),
            running: Rc::new(RefCell::new(true)),
        }
    }

    fn get_coordinate_usize(&self) -> (usize, usize) {
        let coordinate = self.get_coordinate();
        (coordinate.get_row(), coordinate.get_col())
    }

    pub(crate) fn set_objective(&mut self, objective: Objective) {
        self.objective = objective;
        println!("{}", format!("New objective: {}", self.objective).color(Color::BrightYellow));
    }

    fn set_next(&mut self, objective: Objective) {
        self.next = objective
    }

    fn next_objective(&mut self) {
        self.objective = self.next.clone();
        self.next = Objective::None;
    }

    fn retrieve_tent(&mut self, world: &mut World) {
        // collect the tent
        if let Some(direction) = self.face_target(world, true, |tile| if let Tent(_) = tile.content { true } else { false },) {
            if let Ok(_) = destroy(self, world, direction) {
                println!("tent retrieved");
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
                println!("new tent crafted");
            } else {
                println!("need materials to craft a new tent");
                println!("I'll just sleep here for today");
                return Ok(());
            }
        }

        let direction = self.face_target(world, true, |tile| {
            if let Content::None | Content::Tree(_) | Content::Rock(_) | Content::Coin(_) | Content::Fish(_) =
                tile.content
            {
                tile.tile_type.properties().can_hold(&Tent(0)) && tile.tile_type.properties().walk()
            } else {
                false
            }
        });

        println!("looking for a place to put the tent.. ");
        // tries to place the tent in the obtained direction
        return if let Some(direction) = direction.as_ref() {
            match put(self, world, Tent(0), 1, direction.clone()) {
                | Ok(_) => {
                    // if it managed to place the tent, it goes inside
                    let _ = go(self, world, direction.clone());
                    Ok(())
                }
                | Err(LibError::MustDestroyContentFirst) => {
                    // if there is some content, break it and put the tent in its place
                    let _ = destroy(self, world, direction.clone());
                    let _ = put(self, world, Tent(0), 1, direction.clone());
                    let _ = go(self, world, direction.clone());
                    Ok(())
                }
                | _ => Err(()),
            }
        } else {
            match Spyglass::new(
                self.get_coordinate().get_row(),
                self.get_coordinate().get_row(),
                20,
                robot_map(world).unwrap().len(),
                None,
                false,
                0.5,
                |tile| tile.tile_type.properties().can_hold(&Tent(0)) && tile.tile_type.properties().walk(),
            )
                .new_discover(self, world)
            {
                | SpyglassResult::Complete | SpyglassResult::Paused | SpyglassResult::Failed(_) => {
                    println!("I'll just sleep here");
                    Ok(())
                }
                | SpyglassResult::Stopped(vec) => {
                    println!("{}", format!("{:?} found at ({},{})", vec[0].0, vec[0].1, vec[0].2).color(Color::BrightGreen));
                    self.compass
                        .set_destination(Destination::Coordinate((vec[0].1, vec[0].2)));
                    self.set_next(Sleeping);
                    self.set_objective(Moving(false));
                    Err(())
                }
            }
        };
    }

    // sets a destination in the zone that is recognized as least explored
    fn set_random_destination(&mut self, world: &mut World) {
        let mut map = robot_map(world).unwrap();
        let mut dim = map.len();

        // decide the precision (iterations of the loop) to apply to the search function
        // based on the map length
        let precision = random::<u32>() % dim.ilog2() + 1;
        println!("precision: {precision}");

        // setup to find the least explored
        let mut target = (dim / 2, dim / 2);
        let mut min = u32::MAX;
        let mut min_quadrant = 0;

        for _ in 1..=precision {
            println!("{}x{} quadrants", dim / 2, dim / 2);
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
                if sum <= min {
                    min = sum;
                    min_quadrant = i;
                }
            }
            println!("quadrant {min_quadrant} is the least explored");

            // finally set as target coordinate the coordinate in the center of the quadrant
            target.0 = match min_quadrant {
                | 0 | 1 => target.0 - dim / 4,
                | 2 | 3 => target.0 + dim / 4,
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

        println!("random destination set: {:?}", target);
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
        self.set_objective(Moving(discover_new));
        // if the weather is good, find the most loaded location
        // (assume it might be further away)
        if let Ok(WeatherType::Sunny) = next_weather {
            if let Ok(c) = self.map.find_most_loaded(world, self, target_content.clone()) {
                self.compass
                    .set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                println!(
                    "found the most {target_content} at {:?} in the map",
                    swap_coordinates(c.into())
                );
            } else {
                println!("{}", format!("{target_content} not found in the map").color(Color::BrightRed));
                self.set_objective(Exploring);
            }
        }
        // otherwise stick to the closest location, so that the bot doesn't go too far off the presumed safe spot it's in
        else if let Ok(c) = self.map.find_closest(world, self, target_content.clone()) {
            self.compass
                .set_destination(Destination::Coordinate(swap_coordinates(c.into())));
            println!("{}",
                     format!(
                         "found the closest {target_content} at {:?} in the map",
                         swap_coordinates(c.into())
                     ).color(Color::BrightGreen)
            );
        } else {
            println!("{}", format!("{target_content} not found in the map").color(Color::BrightRed));
            self.set_objective(Exploring);
        }
    }

    // if a target is present in the 9x9 around the robot, it makes sure the robot is actually facing it
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

    fn look_ahead(&self, direction: Direction) -> (usize, usize) {
        let (row, col) = self.get_coordinate_usize();
        match direction {
            Up => (row - 1, col),
            Down => (row + 1, col),
            Left => (row, col - 1),
            Right => (row, col + 1),
        }
    }

    // blindly walks towards the current destination
    fn move_towards(&mut self, world: &mut World) {
        if let Some(Destination::Coordinate((dest_row, dest_col))) =
            *self.compass.get_destination()
        {
            // try to take between 2 and 8 steps  going blindly towards the destination
            let steps = random::<u8>() % 4 + 1;
            let (mut stuck_row, mut stuck_col) = (false, false);
            println!("following my heart and not my compass");
            for _ in 1..=steps {
                let (curr_row, curr_col) = self.get_coordinate_usize();
                if curr_row < dest_row {
                    if go(self, world, Down).is_err() {
                        stuck_row = true;
                    }
                } else if curr_row > dest_row {
                    if go(self, world, Up).is_err() {
                        stuck_row = true;
                    }
                }
                if curr_col < dest_col {
                    if go(self, world, Right).is_err() {
                        stuck_col = true;
                    }
                } else if curr_col > dest_col {
                    if go(self, world, Left).is_err() {
                        stuck_col = true;
                    }
                }

                if stuck_row && stuck_col {
                    break;
                } else {
                    self.gui.as_mut().map(|gui| {
                        gui.update_world(robot_map(world).unwrap());
                    });
                }
            }
            self.compass.clear_destination();
            self.compass
                .set_destination(Destination::Coordinate((dest_row, dest_col)));
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
        if let Night = look_at_sky(world).get_time_of_day() {
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
                // retrieves the tent if it's not already in the inventory
                if *self.get_backpack().get_contents().get(&Tent(0)).unwrap() == 0usize {
                    self.retrieve_tent(world);
                }

                let mut pilot_objective = Objective::None;
                if assisted {
                    // get the objective chosen by the pilot
                    if let Some(pilot) = self.pilot.as_mut() {
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

                // if there is no pilot, or they decided not to intervene,
                // go on autopilot and let the AI decide what to do
                if !assisted || pilot_objective == Objective::None {
                    let weather = look_at_sky(world).get_weather_condition();
                    let next_weather = self.forecast.predict_from_time(0, 24);

                    // if current weather is bad, try sleeping for the day
                    if let WeatherType::TrentinoSnow | WeatherType::TropicalMonsoon = weather {
                        println!("The weather today is {weather:?}");
                        self.set_objective(Sleeping);
                    }
                    // if the weather for the next day is bad, move close to a town
                    else if let Ok(WeatherType::TrentinoSnow | WeatherType::TropicalMonsoon) = next_weather.as_ref() {
                        self.set_objective(Moving(true));
                        println!("The weather tomorrow is {:?}", next_weather.as_ref().unwrap());
                        print!("Decided to reach shelter from tomorrow's storm and ");
                        if let Ok(c) = self.map.find_closest(world, self, Content::Building) {
                            println!("found some buildings");
                            self.compass
                                .set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                        } else if let Ok(c) = self.map.find_closest(world, self, Content::Market(0)) {
                            println!("found a market");
                            self.compass
                                .set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                        } else if let Ok(c) = self.map.find_closest(world, self, Content::Bank(0..0)) {
                            println!("found a bank");
                            self.compass
                                .set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                        } else if let Ok(c) = self.map.find_closest(world, self, Content::Tree(0)) {
                            println!("found a tree");
                            self.compass
                                .set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                            self.set_objective(Moving(false));
                        } else {
                            println!("found none! to exploring then");
                            self.set_objective(Exploring)
                        }

                        // wait till the night at the shelter
                        self.set_next(Waiting(Night));
                    }
                    // if the backpack is more than 80% full, go to the market and sell
                    else if self.get_backpack().get_contents().values().sum::<usize>()
                        >= self.get_backpack().get_size() / 5 * 4
                    {
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
                    // if the backpack is less than 50% full, gather some content
                    else if self.get_backpack().get_contents().values().sum::<usize>()
                        <= self.get_backpack().get_size() / 2
                    {
                        // select the item of which the robot holds less
                        let (mut min_content, mut min_quantity) = (Vec::new(), usize::MAX);
                        for (content, quantity) in self.get_backpack().get_contents().iter() {
                            if let Content::Tree(_) | Content::Rock(_) | Content::Fish(_) = content {
                                if *quantity < min_quantity {
                                    min_quantity = *quantity;
                                    min_content.clear();
                                    min_content.push(content.clone());
                                } else if *quantity == min_quantity {
                                    min_content.push(content.clone());
                                }
                            }
                        }

                        let target_content: Content;
                        if min_content.len() == 1 {
                            target_content = min_content[0].clone();
                        } else {
                            let range = min_content.len();
                            // choose randomly if more than one have the same quantity
                            target_content = min_content[random::<usize>() % range].clone();
                        }

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
                // otherwise do what the pilot decided
                else {
                    self.set_next(Objective::None);
                    self.set_objective(pilot_objective);
                }
            }

            | Waiting(target_time) => {
                println!(".");
                let current_time = look_at_sky(world).get_time_of_day();
                if current_time == target_time {
                    if let DayTime::Morning = current_time {
                        println!("{}", "-> time to wake up!".color(Color::BrightYellow));
                    } else {
                        println!("{}", "-> finished waiting".color(Color::BrightCyan));
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
                            println!("picked up some supplies while moving");
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
                        // intervene and manually move towards it as it might mean it's stuck
                        let next = self.look_ahead(direction.clone());
                        if self.last_coords.contains(&next) && random::<u8>() % 3 == 0 {
                            self.move_towards(world);
                        }


                        if let Err(LibError::CannotWalk) = go(self, world, direction.clone()) {
                            println!("can't go {direction:?} from here");

                            // if the robot is moving towards a content
                            if let Gathering(content) = self.next.clone() {
                                println!("next I have to be {:?}", self.next);

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
                                    print!("building a road to the {content}..");
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
                            // if the robot was going to some specific coordinate, make it so
                            // that it won't try to reach it again next tick
                            else if let Some(Destination::Coordinate(c)) = self.compass.get_destination() {
                                // if it's going there to get reach a town, mark it as reached anyway
                                self.pins.insert(*c);
                                self.next_objective();
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
                        println!("destination is {msg}");
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
                println!("{}", format!("charge: {}/{target_level}", self.get_energy().get_energy_level()).color(Color::Cyan));
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
                    println!("{}", "couldn't place the tent".color(Color::BrightRed))
                }
            }

            // the robot needs to gather some type of content
            | Gathering(content) => {
                // at the time of writing this
                // collect all seems not to collect the content you're directly standing on,
                // and since the NLA compass takes you exactly on it, I need to move to face
                // the content and then use collect all
                if let Some(direction) = self.face_target(world, true, |tile| tile.content.to_default() == content.to_default()) {
                    let _ = destroy(self, world, direction);
                }

                // collect all content in the area
                let mut requirements = HashMap::new();
                requirements.insert(content.to_default(), 1);
                CollectAll::collect_items(self, world, 5, requirements);

                let next_weather = self.forecast.predict_from_time(0, 24).unwrap_or(WeatherType::Sunny);

                // if there is still space in the backpack and there is no storm incoming, continue the gathering streak
                if self.get_backpack().get_contents().values().sum::<usize>() < self.get_backpack().get_size() * 4 / 5
                    && next_weather != WeatherType::TrentinoSnow
                    && next_weather != WeatherType::TropicalMonsoon
                {
                    // set the status to MOVING for the next ticks,
                    // in order to move to the next closest area with the target content
                    if let Ok(c) = self.map.find_closest(world, self, content.clone()) {
                        let c = swap_coordinates(c.into());
                        println!("found more {} at ({}, {})", content, c.0, c.1);
                        self.compass.set_destination(Destination::Coordinate(c));
                        self.set_objective(Moving(false));
                    } else {
                        println!("no {} found in the vicinity, need to explore", content);
                        self.set_objective(Exploring);
                    }
                    self.set_next(Gathering(content));
                }
                // otherwise go selling
                else {
                    println!("{}", "backpack too full, selling instead".color(Color::BrightRed));

                    let sellable_content = self.get_content_to_sell();
                    self.set_objective(Selling(sellable_content));
                    self.set_next(Objective::None);
                }
            }

            // the robot needs to sell some type of content
            | Selling(content) => {
                // look for market in the vicinity
                match self.face_target(world, true,
                                       |tile| {
                                           if let Content::Market(n) = tile.content {
                                               n > 0
                                           } else { false }
                                       }) {
                    | None => {
                        if let Ok(c) = self.map.find_closest(world, self, Content::Market(0)) {
                            self.compass
                                .set_destination(Destination::Coordinate(swap_coordinates(c.into())));
                            self.set_next(Selling(content));
                            self.set_objective(Moving(true));
                        } else {
                            println!("{}", "No market found".color(Color::BrightRed));
                            self.set_next(Selling(content));
                            self.set_objective(Exploring);
                        }
                    }
                    | Some(direction) => {
                        if let Some(quantity) = self.get_backpack().get_contents().get(&content) {
                            let _ = put(self, world, content, *quantity, direction);
                            self.next_objective();
                        }
                    }
                }
            }

            | Depositing => {
                // look for bank
                match self.face_target(world, true, |tile| {
                    if let Content::Bank(_) = tile.content {
                        true
                    } else {
                        false
                    }
                }) {
                    | None => {
                        if let Ok(c) = self.map.find_closest(world, self, Content::Bank(0..0)) {
                            self.compass.set_destination(Destination::Coordinate(c.into()));
                            self.set_next(Depositing);
                            self.set_objective(Moving(true));
                        } else {
                            println!("{}", "No bank found".color(Color::BrightRed));
                            self.set_objective(Exploring);
                        }
                    }
                    | Some(direction) => {
                        if let Some(quantity) = self.get_backpack().get_contents().get(&Content::Coin(0)) {
                            let _ = put(self, world, Content::Coin(0), *quantity, direction);
                            self.next_objective();
                        }
                    }
                }
            }

            | Exploring => {
                let dim = robot_map(world).unwrap().len();
                let mut mark_visited = false;

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
                        | Content::Rock(_) => stops_when = |tile| if let Content::Rock(_) = tile.content { true } else { false },
                        | Content::Tree(_) => stops_when = |tile| if let Content::Tree(_) = tile.content { true } else { false },
                        | Content::Fish(_) => stops_when = |tile| if let Content::Fish(_) = tile.content { true } else { false },
                        | _ => {}
                    }
                } else {
                    mark_visited = true;
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
                    for (tile, row, col) in vec.iter() {
                        if !mark_visited || (mark_visited && !self.pins.contains(&(*row, *col))) {
                            println!("{}", format!("{} found at ({}, {})", tile.content, row, col).color(Color::BrightGreen));
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
                println!("No objective set.. deciding what to do next");
                let _ = robot_view(self, world);
                if let Objective::None = self.next {
                    self.set_objective(Praying);
                } else {
                    self.next_objective();
                }
            }
        }
    }

    fn manual_pilot(&mut self, world: &mut World) {
        println!("{}", format!("Energy: {}/1000", self.get_energy().get_energy_level()).color(Color::BrightCyan));

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
        // try to reconnect the pilot, in case the pico got unplugged
        if self.pilot.is_none() {
            self.pilot = Pilot::new().ok();
        }

        match self.pilot.as_ref() {
            Some(pilot) => {
                // if the pilot is manual, get the input action
                if pilot.is_manual() {
                    self.manual_pilot(world);
                }
                // there is a pilot, but it's not manual: go assisted mode
                else {
                    self.auto_pilot(world, true);
                }
            }
            None => {
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
    }

    fn handle_event(&mut self, event: Event) {
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
                println!("score: {}", self.score);
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
                println!("-> Moved to {coords:?}")
            }
            | Event::TileContentUpdated(_, _) => {}
            | Event::AddedToBackpack(content, quantity) => {
                println!("{}", format!("-> {quantity} {content} added to backpack").color(Color::BrightCyan));
            }
            | Event::RemovedFromBackpack(content, quantity) => {
                println!("{}", format!("-> {quantity} {content} removed from backpack").color(Color::BrightMagenta));
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