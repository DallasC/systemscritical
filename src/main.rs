use rand;

use quicksilver::{
    prelude::*, sound, geom,
    graphics::{self, Background, Color, Image},
    lifecycle::{Asset, Settings, State, Window, run},
};

type Point2 = geom::Vector;
type Vector2 = geom::Vector;

/// Create a unit vector representing the
/// given angle (in radians)
fn vec_from_angle(angle: f32) -> Vector2 {
    let vx = angle.sin();
    let vy = angle.cos();
    Vector2::new(vx, vy)
}

/// Just makes a random `Vector2` with the given max magnitude.
fn random_vec(max_magnitude: f32) -> Vector2 {
    let angle = rand::random::<f32>() * 2.0 * std::f32::consts::PI;
    let mag = rand::random::<f32>() * max_magnitude;
    vec_from_angle(angle) * (mag)
}

#[derive(Debug, PartialEq)]
enum ActorType {
    Player,
    Rock,
    Shot,
    Radar,
    Wormhole,
}

#[derive(Debug, PartialEq)]
enum Systems {
    Engines,
    Wepons,
    Radar,
}

#[derive(Debug)]
struct Actor {
    tag: ActorType,
    sys: Systems,
    pos: Point2,
    facing: f32,
    velocity: Vector2,
    ang_vel: f32,
    bbox_size: f32,
    layer: i32,

    // I am going to lazily overload "life" with a
    // double meaning:
    // for shots and radar, it is the time left to live,
    // for players and rocks, it is the actual hit points.
    life: f32,
}

const PLAYER_LIFE: f32 = 1.0;
const SHOT_LIFE: f32 = 2.0;
const RADAR_LIFE: f32 = 3.0;
const ROCK_LIFE: f32 = 1.0;

const PLAYER_BBOX: f32 = 12.0;
const ROCK_BBOX: f32 = 12.0;
const WORMHOLE_BBOX: f32 = 16.0;
const SHOT_BBOX: f32 = 6.0;

const MAX_ROCK_VEL: f32 = 50.0;
const MAX_WORMHOLE_VEL: f32 = 25.0;

fn create_player() -> Actor {
    Actor {
        tag: ActorType::Player,
        sys: Systems::Radar,
        pos: Vector2::ZERO,
        facing: 0.,
        velocity: Vector2::ZERO,
        ang_vel: 0.,
        bbox_size: PLAYER_BBOX,
        layer: 500,
        life: PLAYER_LIFE,
    }
}

fn create_wormhole() -> Actor {
    Actor {
        tag: ActorType::Wormhole,
        sys: Systems::Radar,
        pos: Vector2::ZERO,
        facing: 0.,
        velocity: Vector2::ZERO,
        ang_vel: 0.,
        bbox_size: WORMHOLE_BBOX,
        layer: 495,
        life: PLAYER_LIFE,
    }
}

fn create_rock() -> Actor {
    Actor {
        tag: ActorType::Rock,
        sys: Systems::Radar,
        pos: Vector2::ZERO,
        facing: 0.,
        velocity: Vector2::ZERO,
        ang_vel: 0.,
        bbox_size: ROCK_BBOX,
        layer: 500,
        life: ROCK_LIFE,
    }
}

fn create_shot() -> Actor {
    Actor {
        tag: ActorType::Shot,
        sys: Systems::Radar,
        pos: Vector2::ZERO,
        facing: 0.,
        velocity: Vector2::ZERO,
        ang_vel: SHOT_ANG_VEL,
        bbox_size: SHOT_BBOX,
        layer: 500,
        life: SHOT_LIFE,
    }
}

fn create_radar(layer: i32) -> Actor {
    Actor {
        tag: ActorType::Radar,
        pos: Vector2::ZERO,
        sys: Systems::Radar,
        facing: 0.,
        velocity: Vector2::ZERO,
        ang_vel: SHOT_ANG_VEL,
        bbox_size: SHOT_BBOX,
        layer: layer,
        life: RADAR_LIFE,
    }
}

/// Create the given number of rocks.
/// Makes sure that none of them are within the
/// given exclusion zone (nominally the player)
/// Note that this *could* create rocks outside the
/// bounds of the playing field, so it should be
/// called before `wrap_actor_position()` happens.
fn create_rocks(num: i32, exclusion: Point2, min_radius: f32, max_radius: f32) -> Vec<Actor> {
    assert!(max_radius > min_radius);
    let new_rock = |_| {
        let mut rock = create_rock();
        let r_angle = rand::random::<f32>() * 2.0 * std::f32::consts::PI;
        let r_distance = rand::random::<f32>() * (max_radius - min_radius) + min_radius;
        rock.pos = exclusion + vec_from_angle(r_angle) * r_distance;
        rock.velocity = random_vec(MAX_ROCK_VEL);
        rock
    };
    (0..num).map(new_rock).collect()
}

fn create_wormholes(num: i32, exclusion: Point2, min_radius: f32, max_radius: f32) -> Vec<Actor> {
    assert!(max_radius > min_radius);
    let new_wormhole = |_| {
        let mut wormhole = create_wormhole();
        let r_angle = rand::random::<f32>() * 2.0 * std::f32::consts::PI;
        let r_distance = rand::random::<f32>() * (max_radius - min_radius) + min_radius;
        wormhole.pos = exclusion + vec_from_angle(r_angle) * r_distance;
        wormhole.velocity = random_vec(MAX_WORMHOLE_VEL);
        wormhole
    };
    (0..num).map(new_wormhole).collect()
}

const SHOT_SPEED: f32 = 200.0;
const SHOT_ANG_VEL: f32 = 0.1;

// Acceleration in pixels per second.
const PLAYER_THRUST: f32 = 100.0;
// Rotation in radians per second.
const PLAYER_TURN_RATE: f32 = 3.0;
// Seconds between shots
const PLAYER_SHOT_TIME: f32 = 0.5;
// Seconds between radar pulses
const PLAYER_RADAR_TIME: f32 = 0.4;

fn player_handle_input(actor: &mut Actor, input: &InputState, dt: f32) {
    actor.facing += dt * PLAYER_TURN_RATE * input.xaxis;

    if input.yaxis > 0.0 {
        player_thrust(actor, dt);
    }
}

fn player_thrust(actor: &mut Actor, dt: f32) {
    let direction_vector = vec_from_angle(actor.facing);
    let thrust_vector = direction_vector * (PLAYER_THRUST);
    actor.velocity += thrust_vector * (dt);
}

const MAX_PHYSICS_VEL: f32 = 200.0;

fn update_actor_position(actor: &mut Actor, dt: f32) {
    // Clamp the velocity to the max efficiently
    let norm_sq = actor.velocity.len2();
    if norm_sq > MAX_PHYSICS_VEL.powi(2) {
        actor.velocity = actor.velocity / norm_sq.sqrt() * MAX_PHYSICS_VEL;
    }
    let dv = actor.velocity * (dt);
    actor.pos += dv;
    actor.facing += actor.ang_vel;
}

/// Takes an actor and wraps its position to the bounds of the
/// screen, so if it goes off the left side of the screen it
/// will re-enter on the right side and so on.
fn wrap_actor_position(actor: &mut Actor, sx: f32, sy: f32) {
    // Wrap screen
    let screen_x_bounds = sx / 2.0;
    let screen_y_bounds = sy / 2.0;
    if actor.pos.x > screen_x_bounds {
        actor.pos.x -= sx;
    } else if actor.pos.x < -screen_x_bounds {
        actor.pos.x += sx;
    };
    if actor.pos.y > screen_y_bounds {
        actor.pos.y -= sy;
    } else if actor.pos.y < -screen_y_bounds {
        actor.pos.y += sy;
    }
}

fn handle_timed_life(actor: &mut Actor, dt: f32) {
    actor.life -= dt;
}

/// Translates the world coordinate system, which
/// has Y pointing up and the origin at the center,
/// to the screen coordinate system, which has Y
/// pointing downward and the origin at the top-left,
fn world_to_screen_coords(screen_width: f32, screen_height: f32, point: Point2) -> Point2 {
    let x = point.x + screen_width / 2.0;
    let y = screen_height - (point.y + screen_height / 2.0);
    Point2::new(x, y)
}

struct Assets {
    player_image: Asset<Image>,
    shot_image: Asset<Image>,
    rock_image: Asset<Image>,
    font: Asset<graphics::Font>,
    shot_sound: Asset<sound::Sound>,
    hit_sound: Asset<sound::Sound>,
}

impl Assets {
    fn new() -> quicksilver::Result<Assets> {
        let player_image = Asset::new(Image::load("player.png"));
        let shot_image = Asset::new(Image::load("shot.png"));
        let rock_image = Asset::new(Image::load("astroid.png"));
        let font = Asset::new(graphics::Font::load("DejaVuSerif.ttf"));

        let shot_sound = Asset::new(sound::Sound::load("pew.ogg"));
        let hit_sound = Asset::new(sound::Sound::load("boom.ogg"));

        Ok(Assets {
            player_image,
            shot_image,
            rock_image,
            font,
            shot_sound,
            hit_sound,
        })
    }

    fn actor_image(&mut self, actor: &Actor) -> &mut Asset<Image> {
        match actor.tag {
            ActorType::Player => &mut self.player_image,
            ActorType::Rock => &mut self.rock_image,
            ActorType::Shot => &mut self.shot_image,
            ActorType::Radar => &mut self.rock_image,
            ActorType::Wormhole => &mut self.rock_image,
        }
    }
}

#[derive(Debug)]
struct InputState {
    xaxis: f32,
    yaxis: f32,
    fire: bool,
    radar: bool,
}

impl Default for InputState {
    fn default() -> Self {
        InputState {
            xaxis: 0.0,
            yaxis: 0.0,
            fire: false,
            radar: false,
        }
    }
}

struct MainState {
    player: Actor,
    shots: Vec<Actor>,
    radar: Vec<Actor>,
    rocks: Vec<Actor>,
    wormhole: Vec<Actor>,
    level: i32,
    score: i32,
    assets: Assets,
    screen_width: f32,
    screen_height: f32,
    input: InputState,
    player_shot_timeout: f32,
    player_radar_timeout: f32,
    radar_layer: i32,
}

impl MainState {
    fn new() -> quicksilver::Result<MainState> {
        print_instructions();

        let assets = Assets::new()?;
        let player = create_player();
        let rocks = create_rocks(5, player.pos, 100.0, 250.0);
        let wormhole = create_wormholes(1, player.pos, 100.0, 250.0);

        let window_size = Vector2::new(800.0, 600.0);
        let s = MainState {
            player,
            shots: Vec::new(),
            radar: Vec::new(),
            rocks,
            wormhole,
            level: 0,
            score: 0,
            assets,
            screen_width: window_size.x,
            screen_height: window_size.y,
            input: InputState::default(),
            player_shot_timeout: 0.0,
            player_radar_timeout: 0.0,
            radar_layer: 0,
        };

        Ok(s)
    }

    fn reset(&mut self) {
        self.player = create_player();
        self.shots = Vec::new();
        self.radar = Vec::new();
        self.rocks = create_rocks(5, self.player.pos, 100.0, 250.0);
        self.wormhole = create_wormholes(1, self.player.pos, 100.0, 250.0);
        self.level = 0;
        self.score = 0;
        self.input = InputState::default();
        self.player_shot_timeout = 0.0;
        self.player_radar_timeout = 0.0;
        self.radar_layer = 0;
    }

    fn fire_player_shot(&mut self) {
        self.player_shot_timeout = PLAYER_SHOT_TIME;

        let player = &self.player;
        let mut shot = create_shot();
        shot.pos = player.pos;
        shot.facing = player.facing;
        let direction = vec_from_angle(shot.facing);
        shot.velocity.x = SHOT_SPEED * direction.x;
        shot.velocity.y = SHOT_SPEED * direction.y;

        self.shots.push(shot);

        let _ = self.assets.shot_sound.execute(|s| s.play());
    }

    fn fire_player_radar(&mut self) {
        self.player_radar_timeout = PLAYER_RADAR_TIME;

        let player = &self.player;
        let mut radar = create_radar(self.radar_layer);
        radar.pos = player.pos;
        self.radar_layer = self.radar_layer + 2;

        self.radar.push(radar);

        let _ = self.assets.shot_sound.execute(|s| s.play());
    }

    fn clear_dead_stuff(&mut self) {
        self.shots.retain(|s| s.life > 0.0);
        self.rocks.retain(|r| r.life > 0.0);
        self.radar.retain(|r| r.life > 0.0);
        self.wormhole.retain(|w| w.life > 0.0);
        if self.radar.len() == 0 {
            self.radar_layer = 0
        }
    }

    fn handle_collisions(&mut self) {
        for rock in &mut self.rocks {
            let pdistance = rock.pos - self.player.pos;
            if pdistance.len() < (self.player.bbox_size + rock.bbox_size) {
                self.player.life = 0.0;
            }
            for shot in &mut self.shots {
                let distance = shot.pos - rock.pos;
                if distance.len() < (shot.bbox_size + rock.bbox_size) {
                    shot.life = 0.0;
                    rock.life = 0.0;
                    self.score += 1;

                    let _ = self.assets.hit_sound.execute(|s| s.play());
                }
            }
        }
        for wormhole in &mut self.wormhole {
            let pdistance = wormhole.pos - self.player.pos;
            if pdistance.len() < (self.player.bbox_size + wormhole.bbox_size) {
                wormhole.life = 0.;
            }
        }
    }

    // fn check_for_level_respawn(&mut self) {
    //     if self.rocks.is_empty() {
    //         self.level += 1;
    //         let r = create_rocks(self.level * 2 + 3, self.player.pos, 100.0, 250.0);
    //         self.rocks.extend(r);
    //     }
    // }

    fn check_for_level_end(&mut self) {
        if self.wormhole.is_empty() {
            self.score += 10;
            self.level += 1;
            self.wormhole = create_wormholes(1, self.player.pos, 100.0, 250.0);
            self.rocks = create_rocks(self.level * 2 + 5, self.player.pos, 100.0, 250.0);
        }
    }
}

/// **********************************************************************
/// A couple of utility functions.
/// **********************************************************************

fn print_instructions() {
    println!();
    println!("Welcome to Systems Critical");
    println!();
    println!("How to play:");
    println!("Switch ship systems with 1,2,3");
    println!("1 engines: you can move forward with w");
    println!("2 wepons: fire wepons with w");
    println!("3 rader: scan the surronding area with w");
    println!();
}

fn draw_actor(
    assets: &mut Assets,
    window: &mut Window,
    actor: &Actor,
    world_coords: (f32, f32),
) -> quicksilver::Result<()> {
    let (screen_w, screen_h) = world_coords;
    let pos = world_to_screen_coords(screen_w, screen_h, actor.pos);
    let image = assets.actor_image(actor);
    if actor.tag == ActorType::Radar {
        let scale = ((RADAR_LIFE - actor.life).trunc() + (RADAR_LIFE - actor.life + 1.).fract()) * 10.;
        let transform = geom::Transform::scale((scale, scale));
        window.draw_ex(
            &geom::Circle::new((pos.x, pos.y), 16),
            Background::Col(Color::GREEN),
            transform,
            actor.layer,
        );
        window.draw_ex(
            &geom::Circle::new((pos.x, pos.y), 15),
            Background::Col(Color::BLACK),
            transform,
            actor.layer + 1,
        );
        Ok(())
    } else if actor.tag == ActorType::Wormhole {
        window.draw_ex(
            &geom::Circle::new((pos.x, pos.y), 14),
            Background::Col(Color::PURPLE),
            geom::Transform::IDENTITY,
            actor.layer,
        );
        window.draw_ex(
            &geom::Circle::new((pos.x, pos.y), 12),
            Background::Col(Color::BLACK),
            geom::Transform::IDENTITY,
            actor.layer,
        );
        window.draw_ex(
            &geom::Circle::new((pos.x, pos.y), 2),
            Background::Col(Color::PURPLE),
            geom::Transform::IDENTITY,
            actor.layer,
        );
        Ok(())
    } else {
        image.execute(|i| {
            let transform = geom::Transform::rotate(actor.facing * 180.0 * std::f32::consts::FRAC_1_PI);
            let target_rect = i.area().with_center((pos.x, pos.y));
            window.draw_ex(
                &target_rect,
                Background::Img(&i),
                transform,
                actor.layer,
            );
            Ok(())
        })
    }
}

impl State for MainState {
    fn new() -> quicksilver::Result<Self> {
        MainState::new()
    }
    
    fn update(&mut self, _window: &mut Window) -> quicksilver::Result<()> {
        const DESIRED_FPS: u32 = 60;
        let seconds = 1.0 / (DESIRED_FPS as f32);

        // Update the player state based on the user input.
        player_handle_input(&mut self.player, &self.input, seconds);
        self.player_shot_timeout -= seconds;
        if self.input.fire && self.player_shot_timeout < 0.0 {
            self.fire_player_shot();
        }
        self.player_radar_timeout -= seconds;
        if self.input.radar && self.player_radar_timeout < 0.0 {
            self.fire_player_radar();
        }

        // Update the physics for all actors.
        // First the player...
        update_actor_position(&mut self.player, seconds);
        wrap_actor_position(
            &mut self.player,
            self.screen_width as f32,
            self.screen_height as f32,
        );

        // Then the shots...
        for act in &mut self.shots {
            update_actor_position(act, seconds);
            wrap_actor_position(act, self.screen_width as f32, self.screen_height as f32);
            handle_timed_life(act, seconds);
        }

        // And radar
        for act in &mut self.radar {
            handle_timed_life(act, seconds);
        }

        // And finally the rocks.
        for act in &mut self.rocks {
            update_actor_position(act, seconds);
            wrap_actor_position(act, self.screen_width as f32, self.screen_height as f32);
        }

        // Handle the results of things moving:
        // collision detection, object death, and if
        // we have killed all the rocks in the level,
        // spawn more of them.
        self.handle_collisions();

        self.clear_dead_stuff();

        // self.check_for_level_respawn();
        self.check_for_level_end();
        // Finally we check for our end state.
        // I want to have a nice death screen eventually,
        // but for now we just quit.
        if self.player.life <= 0.0 {
            println!("Your score was {}", self.score);
            println!("Your level was {}", self.level);
            println!("Try Again");
            MainState::reset(self);
        }

        Ok(())
    }

    fn event(&mut self, event: &Event, _window: &mut Window) -> quicksilver::Result<()> {
        match event {
            // Buttons pressed
            Event::Key(Key::Key1, ButtonState::Pressed) => {
                self.player.sys = Systems::Engines;
            }
            Event::Key(Key::Key2, ButtonState::Pressed) => {
                self.player.sys = Systems::Wepons;
            }
            Event::Key(Key::Key3, ButtonState::Pressed) => {
                self.player.sys = Systems::Radar;
            }
            Event::Key(Key::W, ButtonState::Pressed) => {
                if self.player.sys == Systems::Radar {
                    self.input.radar = true;
                } else if self.player.sys == Systems::Wepons {
                    self.input.fire = true;
                } else {
                    self.input.yaxis = 1.0;
                }
            }
            Event::Key(Key::A, ButtonState::Pressed) => {
                self.input.xaxis = -1.0;
            }
            Event::Key(Key::D, ButtonState::Pressed) => {
                self.input.xaxis = 1.0;
            }
            Event::Key(Key::Escape, ButtonState::Pressed) => {
                std::process::exit(0);
            }
            // Buttons released
            Event::Key(Key::W, ButtonState::Released) => {
                self.input.yaxis = 0.0;
                self.input.fire = false;
                self.input.radar = false;
            }
            Event::Key(Key::A, ButtonState::Released) => {
                self.input.xaxis = 0.0;
            }
            Event::Key(Key::D, ButtonState::Released) => {
                self.input.xaxis = 0.0;
            }
            _ => (), // Do nothing
        }
        Ok(())
    }

    fn draw(&mut self, window: &mut Window) -> quicksilver::Result<()> {
        // Clear the screen...
        window.clear(Color::BLACK)?;

        // Loop over all objects drawing them...
        {
            let assets = &mut self.assets;
            let coords = (self.screen_width, self.screen_height);

            let p = &self.player;
            draw_actor(assets, window, p, coords)?;

            for s in &self.shots {
                draw_actor(assets, window, s, coords)?;
            }

            for r in &self.rocks {
                draw_actor(assets, window, r, coords)?;
            }

            for r in &self.radar {
                draw_actor(assets, window, r, coords)?;
            }

            for w in &self.wormhole {
                draw_actor(assets, window, w, coords)?;
            }
        }

        // And draw the GUI elements in the right places.
        let level_dest = Point2::new(100.0, 10.0);
        let score_dest = Point2::new(300.0, 10.0);

        let level_str = format!("Level: {}", self.level);
        let score_str = format!("Score: {}", self.score);

        self.assets.font.execute(|f| {
            let style = FontStyle::new(24.0, Color::WHITE);
            let text = f.render(&level_str, &style)?;
            window.draw(&text.area().with_center(level_dest), Background::Img(&text));

            let text = f.render(&score_str, &style)?;
            window.draw(&text.area().with_center(score_dest), Background::Img(&text));

            Ok(())
        })?;

        Ok(())
    }
}

pub fn main() -> quicksilver::Result<()> {
    run::<MainState>("Systems Critical", Vector::new(800, 600),
        Settings::default()
    );
    Ok(())
}