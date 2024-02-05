extern crate piston_window;
extern crate tokio;

use std::{
    ops::Index,
    sync::{Arc, Mutex},
};

use piston_window::*;
use rand::prelude::*;

use tokio::{sync::oneshot, task::*, time::*};

#[derive(Debug)]
pub struct Particle {
    pub point: [f64; 2],
    pub velocity: [f64; 2],
    pub acceleration: [f64; 2],
    radius: f64,
    pub active: bool,
    pub color: [f32; 4],
}

impl Particle {
    pub fn new(point: [f64; 2]) -> Self {
        Self {
            point,
            velocity: [0.0, 0.0],
            acceleration: [0.0, 0.0],
            active: true,
            radius: 1.0,
            color: [
                rand::thread_rng().gen(),
                rand::thread_rng().gen(),
                rand::thread_rng().gen(),
                1.0,
            ],
        }
    }

    pub fn with_velocity(self, velocity: [f64; 2]) -> Self {
        Self { velocity, ..self }
    }

    pub fn push(&mut self, acceleration: [f64; 2]) {
        self.acceleration[0] += acceleration[0];
        self.acceleration[1] += acceleration[1];
    }

    pub fn run(&mut self) {
        self.velocity[0] += self.acceleration[0];
        self.velocity[1] += self.acceleration[1];

        self.point[0] += self.velocity[0];
        self.point[1] += self.velocity[1];

        self.acceleration[0] = 0.0;
        self.acceleration[1] = 0.0;
    }

    pub fn draw<G>(&self, draw_state: &DrawState, transform: [[f64; 3]; 2], g: &mut G)
    where
        G: graphics::Graphics,
    {
        Line::new(self.color.into(), self.radius).draw(
            [
                self.point[0],
                self.point[1],
                self.point[0] + self.radius,
                self.point[1] + self.radius,
            ],
            &draw_state,
            transform,
            g,
        );
    }

    pub fn get_geometry(&self) -> [f64; 4] {
        [
            self.point[0],
            self.point[1],
            self.point[0] + self.radius,
            self.point[1] + self.radius,
        ]
    }
}

pub struct GravityHandler {
    pub entities: Vec<Arc<Mutex<Particle>>>,
}

impl GravityHandler {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
        }
    }

    pub fn run(&mut self) {
        self.entities.retain(|test| test.lock().unwrap().active);
        self.entities.iter().for_each(|entity| {
            let rc = entity.clone();
            let mut ent = rc.lock().unwrap();
            ent.push([0.0, 0.098]);
            ent.run();
        });
    }

    pub fn draw<G>(&mut self, draw_state: &DrawState, transform: [[f64; 3]; 2], g: &mut G)
    where
        G: graphics::Graphics,
    {
        self.entities.iter().for_each(|particle| {
            let rc = particle.clone();
            let part = rc.lock().unwrap();

            part.draw(draw_state, transform, g);
        });
    }

    pub fn spawn_one(&mut self, point: [f64; 2]) -> Arc<Mutex<Particle>> {
        let new = Arc::new(Mutex::new(Particle::new(point).with_velocity([
            rand::thread_rng().gen_range(-1.0..1.0),
            rand::thread_rng().gen_range(-1.0..1.0),
        ])));
        self.entities.push(new.clone());

        let (tx, rx) = oneshot::channel();

        let rc = new.clone();

        let rand: f64 = rand::thread_rng().gen::<f64>() * 5000.0;

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(rand as u64)).await;
            tx.send(rc).unwrap();
        });

        tokio::spawn(async move {
            let part = rx.await.unwrap();
            part.lock().unwrap().active = false;
        });

        new.clone()
    }
}

pub struct Solid {
    pub line: Line,
    pub geometry: [f64; 4],
    pub threshold: f64,
}

impl Solid {
    pub fn new(geometry: [f64; 4], radius: f64) -> Self {
        Self {
            line: Line::new([0.3, 0.5, 0.6, 1.0], radius),
            geometry,
            threshold: 2.0,
        }
    }

    pub fn run(&mut self) {}

    pub fn draw<G>(&mut self, draw_state: &DrawState, transform: [[f64; 3]; 2], g: &mut G)
    where
        G: graphics::Graphics,
    {
        self.line.draw(self.geometry, &draw_state, transform, g);
    }

    pub fn is_colliding(&self, geometry: [f64; 4]) -> bool {
        let x_check = geometry[0] >= self.geometry[0] && geometry[2] <= self.geometry[2];
        let y_check = geometry[1] >= self.geometry[1] - self.threshold
            && geometry[3] <= self.geometry[3] + self.line.radius;
        return x_check && y_check;
    }
}

pub struct ExplodingParticles {
    pub particles: Vec<Arc<Mutex<Particle>>>,
    pub origin: [f64; 2],
    pub strength: f64,
    pub fading: Duration,
}

impl ExplodingParticles {
    pub fn new() -> Self {
        Self {
            particles: Vec::new(),
            origin: [0.0, 0.0],
            strength: 0.0,
            fading: Duration::from_millis(500),
        }
    }

    pub fn with_origin(self, origin: [f64; 2]) -> Self {
        Self { origin, ..self }
    }

    pub fn with_strength(self, strength: f64) -> Self {
        Self { strength, ..self }
    }

    pub fn trigger(&mut self) {
        for _ in 0..50 {
            let rc = Arc::new(Mutex::new(Particle::new(self.origin).with_velocity([
                rand::thread_rng().gen_range(-self.strength..self.strength),
                rand::thread_rng().gen_range(-self.strength..self.strength),
            ])));
            self.particles.push(rc.clone());
        }

        self.particles.iter().for_each(|parti| {
            let rc = parti.clone();

            let (tx, rx) = oneshot::channel();

            let this_duration = self.fading;
            tokio::spawn(async move {
                tokio::time::sleep(this_duration).await;
                tx.send(rc).unwrap();
            });

            tokio::spawn(async move {
                let part = rx.await.unwrap();

                part.lock().unwrap().active = false;
            });
        });
    }

    pub fn update(&mut self) {
        self.particles.iter().for_each(|parti| {
            let rc = parti.clone();
            rc.lock().unwrap().run();
        });
        self.particles.retain(|part| part.lock().unwrap().active);
    }

    pub fn draw<G: graphics::Graphics>(
        &self,
        draw_state: &DrawState,
        transform: [[f64; 3]; 2],
        g: &mut G,
    ) {
        self.particles.iter().for_each(|part| {
            let rc = part.clone();
            let particle = rc.lock().unwrap();

            particle.draw(draw_state, transform, g);
        });
    }
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let opengl = OpenGL::V4_5;
    let mut window: PistonWindow = WindowSettings::new("shapes", [512; 2])
        .exit_on_esc(true)
        .graphics_api(opengl)
        .build()
        .unwrap();

    let mut handler = GravityHandler::new();

    for _ in 0..10 {
        handler.spawn_one([
            rand::thread_rng().gen::<f64>() * 500.0,
            rand::thread_rng().gen::<f64>() * 500.0,
        ]);
    }

    let mut explosion = ExplodingParticles::new().with_strength(2.0);

    while let Some(e) = window.next() {
        if let Event::Input(test, test2) = &e {
            if let Input::Button(args) = test {
                if args.state == ButtonState::Press {
                    println!("cliked");
                    explosion.trigger();
                    explosion.particles.iter().for_each(|arc| {
                        handler.entities.push(arc.clone());
                    });
                }
            }
        }

        e.mouse_cursor(|take| {
            println!("test");
            /* explosion.origin = take;
                        explosion.trigger();
                        explosion.particles.iter().for_each(|arc| {
                            handler.entities.push(arc.clone());
                        });
            */
            for _ in 0..10 {
                //handler.spawn_one(take);
            }
        });

        window.draw_2d(&e, |c, g, _| {
            clear([1.0; 4], g);

            let mut solid = Solid::new([221.0, 420.0, 500.0, 420.00], 10.0);
            let mut sol2 = Solid::new([45.0, 45.0, 240.0, 240.0], 10.0);

            // Line::new(color, 0.1).draw([45.0, 45.0, 46.0, 46.0], &c.draw_state, c.transform, g);
            handler.entities.iter().for_each(|enti| {
                let mut inner = enti.lock().unwrap();
                let geometry = inner.get_geometry();
                let acceleration = inner.acceleration;
                let vel = inner.velocity;
                if solid.is_colliding(geometry) {
                    let stop = rand::thread_rng().gen_range(12..18) as f64 / 10.0;
                    inner.push([0.0, -vel[1]]);
                }

                if sol2.is_colliding(geometry) {
                    inner.push([0.0, -vel[1]]);
                }
            });
            handler.run();
            explosion.update();

            solid.draw(&c.draw_state, c.transform, g);
            sol2.draw(&c.draw_state, c.transform, g);

            handler.draw(&c.draw_state, c.transform, g);
            explosion.draw(&c.draw_state, c.transform, g);
            /*one.clone()
                .lock()
                .unwrap()
                .draw(&c.draw_state, c.transform, g);

            */
        });
    }
}
