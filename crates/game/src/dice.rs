//! A single d6 in a tray: impulse-based rigid body physics and the two camera views.

use glam::{Mat4, Quat, Vec2, Vec3};

use crate::scene3d::{DIE_HALF, FACES, MeshId, Scene3d, TRAY_X, TRAY_Z};
use crate::{Canvas, Frame, Input, SCREEN_H, SCREEN_W, Sfx, rng::Rng};

const STEP: f32 = 1.0 / 240.0;
const GRAVITY: f32 = -30.0;
const RESTITUTION: f32 = 0.35;
/// Contacts slower than this do not bounce, which lets the die come to rest.
const BOUNCE_SPEED: f32 = 1.0;
const FRICTION: f32 = 0.5;
const INV_MASS: f32 = 1.0;
/// A solid cube's inertia is the same about every axis: m * side^2 / 6.
const INV_INERTIA: f32 = 6.0;
const LINEAR_DAMPING: f32 = 0.1;
const ANGULAR_DAMPING: f32 = 0.8;
/// Each step's gravity leaves a few cm/s of contact jitter, so rest is judged above that.
const REST_SPEED: f32 = 0.25;
const REST_TIME: f32 = 0.25;
/// Contact impulse above which a bounce clacks and rumbles.
const CLACK_IMPULSE: f32 = 1.2;
const CLACK_GAP: f32 = 0.06;
const HISTORY: usize = 8;
const SWIPE_MIN: f32 = 40.0;

struct Plane {
    n: Vec3,
    d: f32,
}

/// The die stays on the side of each plane where dot(n, p) >= d.
const PLANES: [Plane; 5] = [
    Plane { n: Vec3::Y, d: 0.0 },
    Plane { n: Vec3::X, d: -TRAY_X },
    Plane { n: Vec3::NEG_X, d: -TRAY_X },
    Plane { n: Vec3::Z, d: -TRAY_Z },
    Plane { n: Vec3::NEG_Z, d: -TRAY_Z },
];

pub struct Die {
    pub pos: Vec3,
    pub vel: Vec3,
    pub rot: Quat,
    pub ang: Vec3,
    accum: f32,
    still: f32,
}

impl Die {
    pub fn resting() -> Self {
        Self { pos: Vec3::new(0.0, DIE_HALF, 0.0), vel: Vec3::ZERO, rot: Quat::IDENTITY, ang: Vec3::ZERO, accum: 0.0, still: REST_TIME }
    }

    pub fn at_rest(&self) -> bool {
        self.still >= REST_TIME
    }

    /// The value on the face pointing up.
    pub fn value(&self) -> u8 {
        let up = self.rot.inverse() * Vec3::Y;
        FACES.iter().max_by(|a, b| a.0.dot(up).total_cmp(&b.0.dot(up))).map_or(1, |f| f.1)
    }

    /// Advances by `dt` in fixed steps and returns the largest contact impulse.
    pub fn step(&mut self, dt: f32) -> f32 {
        if self.at_rest() {
            return 0.0;
        }
        self.accum += dt;
        let mut impact: f32 = 0.0;
        while self.accum >= STEP {
            self.accum -= STEP;
            impact = impact.max(self.substep(STEP));
        }
        if self.vel.length() < REST_SPEED && self.ang.length() < REST_SPEED && self.pos.y < DIE_HALF * 1.05 {
            self.still += dt;
            if self.at_rest() {
                self.vel = Vec3::ZERO;
                self.ang = Vec3::ZERO;
            }
        } else {
            self.still = 0.0;
        }
        impact
    }

    fn substep(&mut self, h: f32) -> f32 {
        self.vel.y += GRAVITY * h;
        let mut max_j: f32 = 0.0;
        for plane in &PLANES {
            for c in corners() {
                let r = self.rot * c;
                let depth = plane.d - plane.n.dot(self.pos + r);
                if depth <= 0.0 {
                    continue;
                }
                let n = plane.n;
                let vn = (self.vel + self.ang.cross(r)).dot(n);
                if vn < 0.0 {
                    let e = if vn < -BOUNCE_SPEED { RESTITUTION } else { 0.0 };
                    let j = -(1.0 + e) * vn / (INV_MASS + INV_INERTIA * r.cross(n).length_squared());
                    self.apply(r, n * j);
                    max_j = max_j.max(j);

                    let v = self.vel + self.ang.cross(r);
                    let vt = v - n * v.dot(n);
                    let speed = vt.length();
                    if speed > 1e-4 {
                        let t = vt / speed;
                        let jt = (speed / (INV_MASS + INV_INERTIA * r.cross(t).length_squared())).min(FRICTION * j);
                        self.apply(r, -t * jt);
                    }
                }
                self.pos += n * depth * 0.5;
            }
        }
        self.pos += self.vel * h;
        self.rot = (Quat::from_scaled_axis(self.ang * h) * self.rot).normalize();
        self.vel *= 1.0 - LINEAR_DAMPING * h;
        self.ang *= 1.0 - ANGULAR_DAMPING * h;
        max_j
    }

    fn apply(&mut self, r: Vec3, impulse: Vec3) {
        self.vel += impulse * INV_MASS;
        self.ang += r.cross(impulse) * INV_INERTIA;
    }

    fn throw(&mut self, pos: Vec3, vel: Vec3, ang: Vec3) {
        *self = Self { pos, vel, ang, rot: self.rot, accum: 0.0, still: 0.0 };
    }
}

fn corners() -> impl Iterator<Item = Vec3> {
    (0..8).map(|i| Vec3::new(sign(i & 1), sign(i & 2), sign(i & 4)) * DIE_HALF)
}

fn sign(bit: usize) -> f32 {
    if bit == 0 { -1.0 } else { 1.0 }
}

/// Maps a bottom-screen point to the tray floor, matching the overhead camera roughly.
fn to_tray([x, y]: [f32; 2]) -> Vec2 {
    Vec2::new((x / SCREEN_W - 0.5) * 2.0 * TRAY_X, (y / SCREEN_H - 0.5) * 2.0 * TRAY_Z)
}

struct Swipe {
    start: [f32; 2],
    last: [f32; 2],
    t: f32,
}

pub struct Dice {
    die: Die,
    rng: Rng,
    history: Vec<u8>,
    swipe: Option<Swipe>,
    clack_cooldown: f32,
    rolling: bool,
}

/// What a frame of the dice scene asks of the game: sounds, plus a short rumble on hard bounces.
pub struct Feedback {
    pub rumble: bool,
}

impl Dice {
    /// Opens the scene with the die already thrown.
    pub fn new(rng: &mut Rng) -> Self {
        let mut dice = Self { die: Die::resting(), rng: Rng::new(rng.next_u32() as u64), history: Vec::new(), swipe: None, clack_cooldown: 0.0, rolling: true };
        dice.random_throw();
        dice
    }

    fn random_throw(&mut self) {
        let x = self.rng.range(-2.5, 2.5);
        let pos = Vec3::new(x, 2.5, TRAY_Z - 1.0);
        let vel = Vec3::new(-x * 0.5 + self.rng.range(-1.0, 1.0), 6.0, self.rng.range(-4.5, -2.0));
        let spin = self.spin();
        self.die.throw(pos, vel, spin);
    }

    fn swipe_throw(&mut self, from: [f32; 2], to: [f32; 2], t: f32) {
        let start = to_tray(from);
        let delta = (to_tray(to) - start) / t.max(0.05);
        let vel = delta.clamp_length_max(14.0);
        let spin = self.spin();
        self.die.throw(Vec3::new(start.x, 2.0, start.y), Vec3::new(vel.x, 4.0, vel.y), spin);
    }

    fn spin(&mut self) -> Vec3 {
        let r = &mut self.rng;
        Vec3::new(r.range(-1.0, 1.0), r.range(-1.0, 1.0), r.range(-1.0, 1.0)).normalize_or(Vec3::X) * r.range(18.0, 30.0)
    }

    pub fn update(&mut self, input: &Input, dt: f32, sfx: &mut Vec<Sfx>) -> Feedback {
        match (&mut self.swipe, input.touch) {
            (None, Some(p)) if input.touch_started => self.swipe = Some(Swipe { start: p, last: p, t: 0.0 }),
            (Some(s), Some(p)) => {
                s.last = p;
                s.t += dt;
            }
            (Some(_), None) => {
                let s = self.swipe.take().unwrap();
                let moved = Vec2::from_array(s.last).distance(Vec2::from_array(s.start));
                if moved > SWIPE_MIN { self.swipe_throw(s.start, s.last, s.t) } else { self.random_throw() }
                self.rolling = true;
                sfx.push(Sfx::Prompt);
            }
            _ => {}
        }
        if input.pressed.any(crate::Buttons::A) {
            self.random_throw();
            self.rolling = true;
            sfx.push(Sfx::Prompt);
        }

        self.clack_cooldown -= dt;
        let impact = self.die.step(dt);
        let clack = impact > CLACK_IMPULSE && self.clack_cooldown <= 0.0;
        if clack {
            self.clack_cooldown = CLACK_GAP;
            sfx.push(Sfx::Clack((impact * 10.0).min(255.0) as u8));
        }
        if self.rolling && self.die.at_rest() {
            self.rolling = false;
            let v = self.die.value();
            self.history.insert(0, v);
            self.history.truncate(HISTORY);
            sfx.push(Sfx::Tick(v * 2));
        }
        Feedback { rumble: clack }
    }

    pub fn draw(&self, frame: &mut Frame) {
        let die = Mat4::from_rotation_translation(self.die.rot, self.die.pos);
        let lift = (self.die.pos.y - DIE_HALF).max(0.0);
        let shadow_r = 0.75 * (1.0 - (lift / 6.0).min(0.6));
        let shadow = Mat4::from_translation(Vec3::new(self.die.pos.x, 0.02, self.die.pos.z)) * Mat4::from_scale(Vec3::splat(shadow_r));
        let draws = vec![(MeshId::Tray, Mat4::IDENTITY), (MeshId::Shadow, shadow), (MeshId::Die, die), (MeshId::Pips, die)];
        let light = Vec3::new(-0.4, 1.0, 0.35).normalize();

        let overhead = Vec3::new(0.0, 9.6, 1.6);
        frame.bottom.scene = Some(scene(overhead, Vec3::new(0.0, 0.0, 0.15), 40.0, light, draws.clone()));

        let target = self.die.pos;
        let chase = Vec3::new(target.x * 0.6, target.y + 3.2, target.z + 3.6);
        frame.top.scene = Some(scene(chase, target, 45.0, light, draws));

        self.draw_hud(&mut frame.top, &mut frame.bottom);
    }

    fn draw_hud(&self, top: &mut Canvas, bottom: &mut Canvas) {
        let white = [1.0, 1.0, 1.0, 1.0];
        match (self.rolling, self.history.first()) {
            (true, _) => top.text(40.0, 40.0, 10.0, white, "ROLLING"),
            (false, Some(v)) => top.text(40.0, 40.0, 10.0, white, &format!("ROLLED {v}")),
            (false, None) => top.text(40.0, 40.0, 10.0, white, "ROLL THE DIE"),
        }
        let past: Vec<String> = self.history.iter().skip(1).map(u8::to_string).collect();
        if !past.is_empty() {
            top.text(40.0, 110.0, 5.0, [1.0, 1.0, 1.0, 0.6], &format!("BEFORE: {}", past.join(" ")));
        }
        bottom.text_centered(SCREEN_H - 34.0, 5.0, [1.0, 1.0, 1.0, 0.7], "TAP SWIPE OR A TO ROLL   BACK: MENU");
    }
}

fn scene(eye: Vec3, target: Vec3, fov_deg: f32, light: Vec3, draws: Vec<(MeshId, Mat4)>) -> Scene3d {
    let view = Mat4::look_at_rh(eye, target, Vec3::Y);
    let proj = Mat4::perspective_rh_gl(fov_deg.to_radians(), SCREEN_W / SCREEN_H, 1.0, 30.0);
    Scene3d { view_proj: proj * view, eye, light, draws }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(die: &mut Die) {
        for _ in 0..(20 * 60) {
            die.step(1.0 / 60.0);
            if die.at_rest() {
                return;
            }
        }
        panic!("die never settled: pos {:?} vel {:?} ang {:?}", die.pos, die.vel, die.ang);
    }

    #[test]
    fn a_thrown_die_settles_flat_inside_the_tray() {
        for seed in 1..20 {
            let mut rng = Rng::new(seed);
            let mut dice = Dice::new(&mut rng);
            dice.random_throw();
            settle(&mut dice.die);
            let p = dice.die.pos;
            assert!((p.y - DIE_HALF).abs() < 0.05, "seed {seed}: resting height {}", p.y);
            assert!(p.x.abs() <= TRAY_X && p.z.abs() <= TRAY_Z, "seed {seed}: left the tray at {p:?}");
            let up = dice.die.rot.inverse() * Vec3::Y;
            assert!(FACES.iter().any(|(n, _)| n.dot(up) > 0.98), "seed {seed}: not flat, up {up:?}");
        }
    }

    #[test]
    fn rolls_cover_every_face() {
        let mut seen = [false; 7];
        let mut rng = Rng::new(42);
        let mut dice = Dice::new(&mut rng);
        for _ in 0..60 {
            dice.random_throw();
            settle(&mut dice.die);
            seen[dice.die.value() as usize] = true;
        }
        assert!(seen[1..].iter().all(|&s| s), "faces seen: {seen:?}");
    }

    #[test]
    fn value_reads_the_upward_face() {
        let mut die = Die::resting();
        assert_eq!(die.value(), 1);
        die.rot = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
        assert_eq!(die.value(), 3);
    }
}
