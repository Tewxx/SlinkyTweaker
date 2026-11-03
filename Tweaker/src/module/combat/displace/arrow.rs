pub fn wrap_degrees(v: f32) -> f32 {
    let v = v % 360.0;
    if v >= 180.0 {
        v - 360.0
    } else if v < -180.0 {
        v + 360.0
    } else {
        v
    }
}

const FADE: f64 = 0.25;
const NUDGE: f64 = 0.32;
const SIZE: f64 = 0.32;

#[derive(Clone, Copy, Default)]
pub struct Target {
    pub id: i32,
    pub position: [f64; 3],
    pub prev_position: [f64; 3],
    pub half_width: f64,
}

#[derive(Default)]
pub struct Arrow {
    has_target: bool,
    visible: bool,
    target: Target,
    yaw: f32,
    prev_yaw: f32,
    fade_from: f32,
    fade_to: f32,
    fade_start: f64,
    nudge_start: Option<f64>,
}

impl Arrow {
    fn alpha(&self, now: f64) -> f32 {
        let t = ((now - self.fade_start) / FADE).clamp(0.0, 1.0);
        self.fade_from + (self.fade_to - self.fade_from) * t as f32
    }

    fn nudge_progress(&self, now: f64) -> Option<f64> {
        self.nudge_start
            .filter(|_| self.has_target)
            .map(|start| ((now - start) / NUDGE).clamp(0.0, 1.0))
    }

    fn nudge_offset(progress: Option<f64>) -> f64 {
        progress.map_or(0.0, |p| (1.0 - (-6.0 * p).exp()) / (1.0 - (-6.0f64).exp()) * 0.3)
    }

    fn nudge_alpha(progress: Option<f64>) -> f32 {
        match progress {
            Some(p) if p > 0.58 => {
                let t = (p - 0.58) / 0.42;
                let floor = (-5.0f64).exp();
                (((-5.0 * t).exp() - floor) / (1.0 - floor)) as f32
            }
            _ => 1.0,
        }
    }

    fn show(&mut self, target: Target, yaw: f32, now: f64) {
        let changed = !self.has_target || self.target.id != target.id;
        let alpha = if changed { 0.0 } else { self.alpha(now) };
        if changed {
            self.nudge_start = None;
            self.prev_yaw = yaw;
        } else {
            self.prev_yaw = self.yaw;
        }
        self.has_target = true;
        self.target = target;
        self.yaw = yaw;
        if !self.visible || changed {
            self.fade_from = alpha;
            self.fade_to = 1.0;
            self.fade_start = now;
        }
        self.visible = true;
    }

    fn hide(&mut self, now: f64) {
        if !self.has_target || !self.visible {
            return;
        }
        self.fade_from = self.alpha(now);
        self.fade_to = 0.0;
        self.fade_start = now;
        self.visible = false;
        self.target.prev_position = self.target.position;
        self.prev_yaw = self.yaw;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, aim: Option<(Target, f32)>, nudge: bool, now: f64) {
        match aim {
            Some((target, yaw)) => self.show(target, yaw, now),
            None => self.hide(now),
        }
        if let Some(start) = self.nudge_start {
            if now - start >= NUDGE {
                self.nudge_start = None;
                if self.visible {
                    self.fade_from = 0.0;
                    self.fade_to = 1.0;
                    self.fade_start = now;
                }
            }
        }
        if nudge && self.visible && self.nudge_start.is_none() {
            self.nudge_start = Some(now);
        }
        if !self.visible && self.alpha(now) <= 0.0 {
            self.clear();
        }
    }

    pub unsafe fn render(&self, origin: [f64; 3], partial: f32, accent: [f32; 3], now: f64) {
        if !self.has_target {
            return;
        }
        let progress = self.nudge_progress(now);
        let alpha = self.alpha(now) * Self::nudge_alpha(progress);
        if alpha <= 0.0 {
            return;
        }
        let t = self.target;
        let feet: [f64; 3] = std::array::from_fn(|i| t.prev_position[i] + (t.position[i] - t.prev_position[i]) * partial as f64);
        let yaw = self.prev_yaw + wrap_degrees(self.yaw - self.prev_yaw) * partial;
        let radians = (yaw as f64).to_radians();
        let reach = t.half_width + 0.3 + SIZE + Self::nudge_offset(progress);
        let centre = [
            feet[0] - radians.sin() * reach - origin[0],
            feet[1] + SIZE + 0.05 - origin[1],
            feet[2] + radians.cos() * reach - origin[2],
        ];
        let shape = pointer();
        unsafe {
            gl::glPushAttrib(gl::ALL_ATTRIB_BITS);
            gl::glPushClientAttrib(gl::CLIENT_ALL_ATTRIB_BITS);
            for cap in [gl::DEPTH_TEST, gl::TEXTURE_2D, gl::LIGHTING, gl::CULL_FACE, gl::ALPHA_TEST] {
                gl::glDisable(cap);
            }
            gl::glEnable(gl::BLEND);
            gl::glBlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::glEnableClientState(gl::VERTEX_ARRAY);
            gl::glVertexPointer(2, gl::FLOAT, 0, shape.as_ptr().cast());
            gl::glPushMatrix();
            gl::glTranslatef(centre[0] as f32, centre[1] as f32, centre[2] as f32);
            gl::glRotatef(270.0 - yaw, 0.0, 1.0, 0.0);
            gl::glRotatef(90.0, 0.0, 0.0, 1.0);
            gl::glScalef(SIZE as f32, SIZE as f32, SIZE as f32);
            gl::glColor4f(accent[0], accent[1], accent[2], alpha);
            gl::glDrawArrays(gl::TRIANGLE_FAN, 0, shape.len() as i32);
            gl::glLineWidth(1.0);
            gl::glEnable(gl::LINE_SMOOTH);
            gl::glColor4f(0.0, 0.0, 0.0, alpha);
            gl::glDrawArrays(gl::LINE_LOOP, 1, shape.len() as i32 - 2);
            gl::glPopMatrix();
            gl::glPopClientAttrib();
            gl::glPopAttrib();
        }
    }
}

fn pointer() -> &'static [[f32; 2]] {
    const QUAD: [[f32; 2]; 4] = [[0.0, -1.0], [-1.0, 1.0], [0.0, 0.5], [1.0, 1.0]];
    const ROUNDING: f32 = 0.4;
    const CORNER_STEPS: usize = 8;
    static SHAPE: std::sync::OnceLock<Vec<[f32; 2]>> = std::sync::OnceLock::new();
    SHAPE.get_or_init(|| {
        let n = QUAD.len();
        let mut outline = Vec::new();
        for i in 0..n {
            let (prev, corner, next) = (QUAD[(i + n - 1) % n], QUAD[i], QUAD[(i + 1) % n]);
            let toward = |to: [f32; 2]| {
                let (dx, dy) = (to[0] - corner[0], to[1] - corner[1]);
                let length = (dx * dx + dy * dy).sqrt();
                let cut = ROUNDING.min(length * 0.45) / length;
                [corner[0] + dx * cut, corner[1] + dy * cut]
            };
            let (a, b) = (toward(prev), toward(next));
            for step in 0..=CORNER_STEPS {
                let t = step as f32 / CORNER_STEPS as f32;
                let (u, v, w) = ((1.0 - t) * (1.0 - t), 2.0 * (1.0 - t) * t, t * t);
                outline.push([u * a[0] + v * corner[0] + w * b[0], u * a[1] + v * corner[1] + w * b[1]]);
            }
        }
        let mut fan = vec![[0.0, 0.0]];
        fan.extend_from_slice(&outline);
        fan.push(outline[0]);
        fan
    })
}

mod gl {
    use std::ffi::c_void;
    pub const ALL_ATTRIB_BITS: u32 = 0x000F_FFFF;
    pub const CLIENT_ALL_ATTRIB_BITS: u32 = 0xFFFF_FFFF;
    pub const DEPTH_TEST: u32 = 0x0B71;
    pub const TEXTURE_2D: u32 = 0x0DE1;
    pub const LIGHTING: u32 = 0x0B50;
    pub const CULL_FACE: u32 = 0x0B44;
    pub const ALPHA_TEST: u32 = 0x0BC0;
    pub const BLEND: u32 = 0x0BE2;
    pub const LINE_SMOOTH: u32 = 0x0B20;
    pub const SRC_ALPHA: u32 = 0x0302;
    pub const ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
    pub const VERTEX_ARRAY: u32 = 0x8074;
    pub const FLOAT: u32 = 0x1406;
    pub const TRIANGLE_FAN: u32 = 0x0006;
    pub const LINE_LOOP: u32 = 0x0002;

    #[link(name = "opengl32")]
    extern "system" {
        pub fn glPushAttrib(mask: u32);
        pub fn glPopAttrib();
        pub fn glPushClientAttrib(mask: u32);
        pub fn glPopClientAttrib();
        pub fn glEnable(cap: u32);
        pub fn glDisable(cap: u32);
        pub fn glBlendFunc(src: u32, dst: u32);
        pub fn glColor4f(r: f32, g: f32, b: f32, a: f32);
        pub fn glLineWidth(width: f32);
        pub fn glEnableClientState(array: u32);
        pub fn glVertexPointer(size: i32, kind: u32, stride: i32, data: *const c_void);
        pub fn glDrawArrays(mode: u32, first: i32, count: i32);
        pub fn glPushMatrix();
        pub fn glPopMatrix();
        pub fn glTranslatef(x: f32, y: f32, z: f32);
        pub fn glRotatef(angle: f32, x: f32, y: f32, z: f32);
        pub fn glScalef(x: f32, y: f32, z: f32);
    }
}
