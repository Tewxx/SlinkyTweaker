use super::blocks::{add, direction, distance, face_centre, normal};
use super::game::{Game, Hit};
use super::hotbar::{Hotbar, Session};
use crate::Result;

pub const REACH: f64 = 4.5;

const FACE_POINTS: [(f64, f64); 17] = [
    (0.0, 0.0),
    (0.2, 0.0),
    (-0.2, 0.0),
    (0.0, 0.2),
    (0.0, -0.2),
    (0.2, 0.2),
    (-0.2, 0.2),
    (0.2, -0.2),
    (-0.2, -0.2),
    (0.4, 0.0),
    (-0.4, 0.0),
    (0.0, 0.4),
    (0.0, -0.4),
    (0.4, 0.4),
    (-0.4, 0.4),
    (0.4, -0.4),
    (-0.4, -0.4),
];

pub fn ray_at(g: &Game, eye: [f64; 3], yaw: f32, pitch: f32, range: f64) -> Result<Option<Hit>> {
    g.ray(eye, add(eye, direction(yaw, pitch), range))
}

pub fn aim_point(g: &Game, eye: [f64; 3], block: [i32; 3], side: i32) -> Result<Option<[f64; 3]>> {
    let n = normal(side);
    let face = face_centre(block, side);
    if (0..3).map(|i| (eye[i] - face[i]) * n[i]).sum::<f64>() <= 0.0 {
        return Ok(None);
    }
    let (u, v) = if n[1] != 0.0 {
        ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0])
    } else if n[2] != 0.0 {
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
    } else {
        ([0.0, 0.0, 1.0], [0.0, 1.0, 0.0])
    };
    for &(a, b) in &FACE_POINTS {
        let point = add(add(face, u, a), v, b);
        if distance(eye, point) > REACH {
            continue;
        }
        let hit = g.ray(eye, add(point, n, -0.05))?;
        if hit.is_some_and(|h| h.block == block && h.side == side) {
            return Ok(Some(point));
        }
    }
    Ok(None)
}

pub struct Placer {
    session: Session,
}

impl Placer {
    pub fn new(hotbar: Hotbar) -> Self {
        Self {
            session: hotbar.session(),
        }
    }
    pub fn active(&self) -> bool {
        self.session.active()
    }
    pub fn hold(&mut self, g: &Game, slot: i32) -> Result<bool> {
        if !self.session.begin(g)? {
            return Ok(false);
        }
        self.session.select(g, slot)?;
        Ok(true)
    }
    pub fn release(&mut self, g: &Game, restore: bool) -> Result<()> {
        self.session.finish(g, restore).map(|_| ())
    }
}
