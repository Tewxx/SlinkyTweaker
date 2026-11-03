use super::layout;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
impl Rect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    pub fn inflate(self, by: f32) -> Self {
        Self {
            x: self.x - by,
            y: self.y - by,
            w: self.w + by * 2.0,
            h: self.h + by * 2.0,
        }
    }
}

static REGISTRY: Mutex<Vec<&'static Element>> = Mutex::new(Vec::new());

pub(crate) fn registered() -> Vec<&'static Element> {
    REGISTRY.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub(crate) struct Element {
    pub id: &'static str,
    pub label: &'static str,
    default: [f32; 2],
    margin: f32,
    drawn: Mutex<Option<Rect>>,
}

impl Element {
    pub const fn corner(id: &'static str, label: &'static str, default: [f32; 2], margin: f32) -> Self {
        Self {
            id,
            label,
            default,
            margin,
            drawn: Mutex::new(None),
        }
    }

    pub fn register(&'static self) {
        let mut registry = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        if !registry.iter().any(|e| std::ptr::eq(*e, self)) {
            registry.push(self);
        }
    }

    fn anchor(&self) -> [f32; 2] {
        layout::get(self.id)
            .unwrap_or(self.default)
            .map(|v| v.clamp(0.0, 1.0))
    }

    pub fn right_aligned(&self) -> bool {
        self.anchor()[0] > 0.5
    }

    pub fn margin(&self) -> f32 {
        self.margin
    }

    pub fn layout(&self, [sw, sh]: [f32; 2], [w, h]: [f32; 2]) -> [f32; 2] {
        let [ax, ay] = self.anchor();
        let margin = self.margin();
        let x = if ax > 0.5 { ax * sw - w - margin } else { ax * sw + margin };
        let y = if ay > 0.5 { ay * sh - h - margin } else { ay * sh + margin };
        let x = x.clamp(0.0, (sw - w).max(0.0));
        let y = y.clamp(0.0, (sh - h).max(0.0));
        *self.drawn.lock().unwrap_or_else(|e| e.into_inner()) = Some(Rect { x, y, w, h });
        [x, y]
    }

    pub(super) fn take_drawn(&self) -> Option<Rect> {
        self.drawn.lock().unwrap_or_else(|e| e.into_inner()).take()
    }

    pub(super) fn place(&self, r: Rect, [sw, sh]: [f32; 2]) {
        if sw <= 0.0 || sh <= 0.0 {
            return;
        }
        let margin = self.margin();
        let axis = |pos: f32, size: f32, limit: f32| {
            let far = pos + size * 0.5 > limit * 0.5;
            let v = (if far { pos + size + margin } else { pos - margin } / limit).clamp(0.0, 1.0);
            if far { v.max(0.501) } else { v.min(0.5) }
        };
        let anchor = [axis(r.x, r.w, sw), axis(r.y, r.h, sh)];
        layout::set(self.id, anchor);
    }

    pub(super) fn reset(&self) {
        layout::remove(self.id);
    }
}
