use super::account;
use super::data::{self, Module};
use super::record::{self, Item, Widget};
use crate::gui::hud_editor::input::Mouse;
use crate::gui::nanovg::{Color, Font, Frame, ALIGN_CENTER, ALIGN_LEFT, ALIGN_MIDDLE, ALIGN_RIGHT, BUTT, MITER, ROUND};
use std::collections::HashMap;
use std::sync::atomic::Ordering;

const W: f32 = 750.0;
const H: f32 = 600.0;
const PROFILES: usize = 5;
const SETTINGS: usize = 6;
const GLOBALS: [&str; 2] = ["GUI", "Friends"];
const TABS: [(&str, &str); 7] = [
    ("Combat", "\u{f140}"),
    ("Move", "\u{f70c}"),
    ("Visual", "\u{f06e}"),
    ("Block", "\u{f1b2}"),
    ("Utility", "\u{f0ad}"),
    ("Profiles", "\u{f0c2}"),
    ("Settings", "\u{f013}"),
];
const CONTENT: R = R { x: 175.0, y: 25.0, w: 575.0, h: 575.0 };
const VIEWPORT: R = R { x: 213.0, y: 116.0, w: 498.0, h: 464.0 };
const CARD_STEP: f32 = 85.0;
const DELETE_HOLD: f32 = 1.0;
const TEXT_LIMIT: usize = 32;

const VK_BACK: usize = 0x08;
const VK_RETURN: usize = 0x0d;
const VK_ESCAPE: usize = 0x1b;
const VK_DELETE: usize = 0x2e;

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
}
fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0)
}
fn gray(v: f32) -> Color {
    let v = (v / 255.0).clamp(0.0, 1.0);
    Color::rgba(v, v, v, 1.0)
}
fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let m = |x: f32, y: f32| x + (y - x) * t;
    Color::rgba(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b), m(a.a, b.a))
}
fn faded(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}

const TEXT: Color = Color::rgba(0.92, 0.92, 0.92, 1.0);
const DARK: Color = Color::rgba(0.071, 0.071, 0.071, 1.0);

#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct R {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
impl R {
    const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    fn contains(self, [x, y]: [f32; 2]) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
    fn intersect(self, o: R) -> R {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        R::new(x, y, ((self.x + self.w).min(o.x + o.w) - x).max(0.0), ((self.y + self.h).min(o.y + o.h) - y).max(0.0))
    }
}

pub(in crate::gui::hud_editor) struct Input {
    pub mouse: Mouse,
    pub wheel: f32,
    pub keys: Vec<(usize, bool)>,
    pub chars: Vec<char>,
    pub extra: u32,
}

#[derive(Default)]
pub(in crate::gui::hud_editor) struct Output {
    pub typing: bool,
    pub close: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Search,
    Number { descriptor: usize, widget: Widget, high: bool },
    ProfileName,
    Bind(usize),
    List(usize),
}

#[derive(Clone, Copy)]
enum Drag {
    Tabs,
    Window([f32; 2]),
    Slider { descriptor: usize, widget: Widget, high: bool, track: R },
    Saturation(R),
    Hue(R),
    ListBar { track: R, total: f32 },
    PanelBar { track: R, total: f32 },
}

#[derive(Clone, Copy)]
struct Picker {
    descriptor: usize,
    entry: Option<usize>,
    hue: f32,
    sat: f32,
    val: f32,
}

#[derive(Default)]
pub(in crate::gui::hud_editor) struct Timewarp {
    anims: HashMap<u64, f32>,
    dt: f32,
    time: f32,
    menu: f32,
    category: usize,
    switched: f32,
    scroll: [f32; 7],
    scroll_view: [f32; 7],
    panel: Option<usize>,
    panel_shown: Option<usize>,
    panel_t: f32,
    panel_scroll: f32,
    panel_scroll_view: f32,
    search: String,
    focus: Option<Focus>,
    text: String,
    choice: Option<(usize, R)>,
    choice_t: f32,
    bind_menu: Option<(usize, R)>,
    picker: Option<Picker>,
    drag: Option<Drag>,
    offset: [f32; 2],
    profiles: Vec<String>,
    profiles_loaded: bool,
    active_profile: Option<String>,
    delete_hold: Option<(String, f32)>,
    settings: HashMap<usize, bool>,
    list_heights: HashMap<usize, f32>,
    m: [f32; 2],
    inside: bool,
    clicked: bool,
    right_clicked: bool,
    down: bool,
    consumed: bool,
    blocked: bool,
    accent: Color,
    alpha: f32,
    face_image: Option<(usize, i32, [[u8; 3]; 64])>,
    manager: usize,
}

fn id(a: usize, b: u64) -> u64 {
    (a as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ b
}

impl Timewarp {
    fn ease(&mut self, key: u64, target: f32, rate: f32) -> f32 {
        let dt = self.dt;
        let v = self.anims.entry(key).or_insert(target);
        *v += (target - *v) * (1.0 - (-rate * dt).exp());
        if (*v - target).abs() < 0.0005 {
            *v = target;
        }
        *v
    }

    fn hover(&self, r: R, clip: R) -> bool {
        self.inside && !self.blocked && r.intersect(clip).contains(self.m)
    }

    fn click(&mut self, r: R, clip: R) -> bool {
        if self.clicked && !self.consumed && self.hover(r, clip) {
            self.consumed = true;
            true
        } else {
            false
        }
    }

    fn right_click(&mut self, r: R, clip: R) -> bool {
        if self.right_clicked && !self.consumed && self.hover(r, clip) {
            self.consumed = true;
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        self.menu = 0.0;
        self.focus = None;
        self.choice = None;
        self.bind_menu = None;
        self.picker = None;
        self.drag = None;
        self.delete_hold = None;
        self.profiles_loaded = false;
    }

    pub fn visible(&self) -> bool {
        self.menu > 0.001
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, vg: &mut Frame<'_>, input: Input, open: bool, screen: [f32; 2], gui_scale: f32, manager: usize, dt: f32) -> Output {
        self.dt = dt;
        self.time += dt;
        self.manager = manager;
        self.accent = Color::rgb(crate::slinky::accent_color());
        if open && !self.visible() {
            self.profiles_loaded = false;
        }
        let mut out = Output::default();
        if !open {
            self.reset();
            self.text.clear();
            return out;
        }
        self.menu = (self.menu + dt / 0.18).min(1.0);

        let base = (screen[0] / 1920.0).min(screen[1] / 1080.0) * 1.36 * gui_scale;
        let scale = base.min(screen[0] * 0.96 / W).min(screen[1] * 0.96 / H).max(0.2);
        let max_offset = [(screen[0] - W * scale).abs() * 0.5, (screen[1] - H * scale).abs() * 0.5];
        self.offset = [
            self.offset[0].clamp(-max_offset[0], max_offset[0]),
            self.offset[1].clamp(-max_offset[1], max_offset[1]),
        ];
        let origin = [
            (screen[0] - W * scale) * 0.5 + self.offset[0],
            (screen[1] - H * scale) * 0.5 + self.offset[1],
        ];
        let mouse = input.mouse;
        self.m = [(mouse.x - origin[0]) / scale, (mouse.y - origin[1]) / scale];
        self.inside = mouse.inside;
        self.clicked = mouse.left.pressed;
        self.right_clicked = mouse.right.pressed;
        self.down = mouse.left.down;
        self.consumed = false;
        self.blocked = false;

        self.keyboard(&input, &mut out);
        if mouse.escape {
            self.escape(&mut out);
        }
        if input.extra != 0 {
            if let Some(Focus::Bind(descriptor)) = self.focus {
                let code = if input.extra & 1 != 0 { 3 } else if input.extra & 2 != 0 { 4 } else { 5 };
                self.bind(descriptor, code);
            }
        }
        self.wheel(input.wheel);
        self.dragging(scale);

        if self.clicked || self.right_clicked {
            if matches!(self.focus, Some(Focus::Bind(_))) {
                self.focus = None;
                self.consumed = true;
            }
            if let Some((_, r)) = self.choice {
                if !self.choice_rect(r).contains(self.m) {
                    self.choice = None;
                    self.consumed = true;
                }
            }
            if let Some((_, r)) = self.bind_menu {
                if !r.contains(self.m) {
                    self.bind_menu = None;
                    self.consumed = true;
                }
            }
            if self.picker.is_some() && !Self::picker_rect().contains(self.m) {
                self.picker = None;
                self.consumed = true;
            }
        }
        let popup = self.choice.is_some() || self.bind_menu.is_some() || self.picker.is_some();
        if self.clicked && !self.consumed && matches!(self.focus, Some(Focus::Number { .. } | Focus::ProfileName | Focus::Search)) {
            self.commit_text();
        }

        let ease = 1.0 - (1.0 - self.menu).powi(3);
        vg.save();
        vg.translate(origin[0], origin[1]);
        vg.scale(scale, scale);
        let grow = snap_scale(0.96 + 0.04 * ease);
        vg.translate(W * 0.5, H * 0.5);
        vg.scale(grow, grow);
        vg.translate(-W * 0.5, -H * 0.5);
        self.alpha = ease;
        vg.global_alpha(ease);

        self.blocked = popup;
        self.window(vg);
        self.sidebar(vg);
        vg.save();
        vg.intersect_scissor(CONTENT.x, CONTENT.y, CONTENT.w, CONTENT.h);
        match self.category {
            PROFILES => self.profiles_page(vg),
            SETTINGS => self.settings_page(vg),
            _ => self.module_list(vg),
        }
        self.panel(vg, &mut out);
        vg.restore();
        if self.clicked && !self.consumed && matches!(self.focus, Some(Focus::List(_))) {
            self.focus = None;
            self.text.clear();
        }
        self.blocked = false;
        if self.choice.is_some() {
            self.choice_popup(vg);
        } else {
            self.choice_t = 0.0;
        }
        if self.bind_menu.is_some() {
            self.bind_popup(vg);
        }
        if self.picker.is_some() {
            self.color_picker(vg);
        }
        if self.clicked && !self.consumed && R::new(0.0, 0.0, W, 26.0).contains(self.m) && self.inside {
            self.drag = Some(Drag::Window([mouse.x - self.offset[0], mouse.y - self.offset[1]]));
            self.consumed = true;
        }
        vg.restore();
        if let Some(Drag::Window(grab)) = self.drag {
            self.offset = [mouse.x - grab[0], mouse.y - grab[1]];
        }
        out.typing |= self.focus.is_some();
        out
    }

    fn keyboard(&mut self, input: &Input, out: &mut Output) {
        let Some(focus) = self.focus else {
            return;
        };
        if let Focus::Bind(descriptor) = focus {
            for &(vk, extended) in &input.keys {
                match vk {
                    VK_ESCAPE => self.focus = None,
                    VK_BACK | VK_DELETE => self.bind(descriptor, 0),
                    _ => {
                        let code = data::key_code(vk, extended);
                        if code != 0 {
                            self.bind(descriptor, code);
                        }
                    }
                }
                if self.focus.is_none() {
                    break;
                }
            }
            return;
        }
        for &(vk, _) in &input.keys {
            match vk {
                VK_RETURN => {
                    self.commit_text();
                    return;
                }
                VK_ESCAPE => {
                    if focus == Focus::Search {
                        self.search.clear();
                    }
                    self.focus = None;
                    self.text.clear();
                    return;
                }
                _ => {}
            }
        }
        for &c in &input.chars {
            match c {
                '\u{8}' => {
                    self.text.pop();
                }
                '\u{7f}' => self.text.clear(),
                c if matches!(focus, Focus::List(_)) && !(c.is_ascii_alphanumeric() || c == ' ' || c == '_') => {}
                c if !c.is_control() && self.text.chars().count() < TEXT_LIMIT => self.text.push(c),
                _ => {}
            }
        }
        if focus == Focus::Search {
            self.search = self.text.clone();
        }
        out.typing = true;
    }

    fn escape(&mut self, out: &mut Output) {
        if self.picker.take().is_some() || self.choice.take().is_some() || self.bind_menu.take().is_some() {
            return;
        }
        if self.panel.take().is_some() {
            return;
        }
        out.close = true;
    }

    fn wheel(&mut self, wheel: f32) {
        if wheel == 0.0 || self.choice.is_some() || self.picker.is_some() || self.bind_menu.is_some() {
            return;
        }
        if self.panel.is_some() {
            self.panel_scroll -= wheel * 60.0;
        } else if CONTENT.contains(self.m) {
            self.scroll[self.category] -= wheel * 60.0;
        }
    }

    fn dragging(&mut self, _scale: f32) {
        if !self.down {
            self.drag = None;
            return;
        }
        let Some(drag) = self.drag else {
            return;
        };
        let [mx, my] = self.m;
        match drag {
            Drag::Window(_) => {}
            Drag::Tabs => {
                let i = ((my - 48.3 + 4.0) / 47.5).floor();
                if my >= 44.0 && (0.0..175.0).contains(&mx) && i >= 0.0 && (i as usize) < TABS.len() {
                    self.switch_tab(i as usize);
                }
            }
            Drag::Slider { descriptor, widget, high, track } => {
                let t = ((mx - track.x) / track.w).clamp(0.0, 1.0);
                match widget {
                    Widget::Float => {
                        if let Some(f) = data::float(descriptor) {
                            let v = data::snap(f.min + (f.max - f.min) * t, f.step, f.min.min(f.max), f.max.max(f.min));
                            data::set_float(descriptor, v);
                        }
                    }
                    Widget::Range => {
                        if let Some(r) = data::range(descriptor) {
                            let v = data::snap(r.min + (r.max - r.min) * t, r.step, r.min.min(r.max), r.max.max(r.min));
                            if high {
                                data::set_range(descriptor, r.low, v.max(r.low));
                            } else {
                                data::set_range(descriptor, v.min(r.high), r.high);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Drag::Saturation(r) => {
                if let Some(p) = self.picker.as_mut() {
                    p.sat = ((mx - r.x) / r.w).clamp(0.0, 1.0);
                    p.val = 1.0 - ((my - r.y) / r.h).clamp(0.0, 1.0);
                    let p = *p;
                    apply_picker(p);
                }
            }
            Drag::Hue(r) => {
                if let Some(p) = self.picker.as_mut() {
                    p.hue = ((my - r.y) / r.h).clamp(0.0, 0.999);
                    let p = *p;
                    apply_picker(p);
                }
            }
            Drag::ListBar { track, total } => {
                let visible = CONTENT.h;
                let thumb = (visible / total * track.h).max(14.0);
                let t = ((my - track.y - thumb * 0.5) / (track.h - thumb)).clamp(0.0, 1.0);
                self.scroll[self.category] = t * (total - visible);
            }
            Drag::PanelBar { track, total } => {
                let visible = 460.0;
                let thumb = (visible / total * track.h).max(14.0);
                let t = ((my - track.y - thumb * 0.5) / (track.h - thumb)).clamp(0.0, 1.0);
                self.panel_scroll = t * (total - visible);
            }
        }
    }

    fn bind(&mut self, descriptor: usize, code: i32) {
        let module = module_at(descriptor);
        data::set_keybind(self.manager, &module, code);
        self.focus = None;
    }

    fn commit_text(&mut self) {
        let Some(focus) = self.focus.take() else {
            return;
        };
        let text = std::mem::take(&mut self.text);
        match focus {
            Focus::Search => self.search = text,
            Focus::Number { descriptor, widget, high } => {
                let Some(typed) = data::parse_number(&text) else {
                    return;
                };
                let unclamped = crate::slinky::settings::UNCLAMPED.load(Ordering::Relaxed);
                match widget {
                    Widget::Float => {
                        if let Some(f) = data::float(descriptor) {
                            let raw = typed / f.mult;
                            let v = if unclamped {
                                if f.step > 0.0 { (raw / f.step).round() * f.step } else { raw }
                            } else {
                                data::snap(raw, f.step, f.min.min(f.max), f.max.max(f.min))
                            };
                            data::set_float(descriptor, v);
                        }
                    }
                    Widget::Range => {
                        if let Some(r) = data::range(descriptor) {
                            let v = data::snap(typed / r.mult, r.step, r.min.min(r.max), r.max.max(r.min));
                            if high {
                                data::set_range(descriptor, r.low.min(v), v);
                            } else {
                                data::set_range(descriptor, v, r.high.max(v));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Focus::ProfileName => self.create_profile(&text),
            Focus::Bind(_) => {}
            Focus::List(descriptor) => {
                if let Some(list) = data::list(descriptor, true) {
                    if let Some(name) = suggestions(&list, &text).into_iter().next() {
                        data::list_add(descriptor, &name);
                    }
                }
                self.focus = Some(Focus::List(descriptor));
            }
        }
    }

    fn window(&mut self, vg: &mut Frame<'_>) {
        shadow(vg, R::new(0.0, 0.0, W, H), 18.0, 28.0, 0.47);
        fill(vg, R::new(0.0, 0.0, W, H), 18.0, rgb(22, 22, 22));
        outline(vg, R::new(0.5, 0.5, W - 1.0, H - 1.0), 18.0, 1.0, rgb(8, 8, 8));
        text(vg, 212.6, 237.5, 335.0, Font::Icons, rgb(45, 45, 45), ALIGN_CENTER, "\u{f252}");
        fill(vg, CONTENT, 18.0, rgb(15, 15, 15));
        text(vg, 738.0, 13.0, 15.0, Font::Medium, rgb(100, 100, 100), ALIGN_RIGHT, "slinky");
    }

    fn sidebar(&mut self, vg: &mut Frame<'_>) {
        let cur = self.category;
        let target_y = 48.3 + cur as f32 * 47.5;
        let target_w = text_width(vg, 17.0, Font::Medium, TABS[cur].0) + 53.0;
        let py = self.ease(1, target_y, 18.0);
        let pw = self.ease(2, target_w, 18.0);
        fill(vg, R::new(13.0, py, pw, 39.0), 19.5, self.accent);
        let full = R::new(0.0, 0.0, W, H);
        for (i, (label, icon)) in TABS.iter().enumerate() {
            let y = 48.3 + i as f32 * 47.5;
            let r = R::new(8.0, y, 157.0, 44.0);
            let goal = if i == cur {
                22.0
            } else if self.hover(r, full) {
                180.0
            } else {
                120.0
            };
            let v = self.ease(7000 + i as u64, goal, 25.0);
            let col = gray(v);
            text(vg, 35.0, y + 19.5, 18.0, Font::Icons, col, ALIGN_CENTER, icon);
            text(vg, 59.0, y + 19.5, 17.0, Font::Medium, col, ALIGN_LEFT, label);
            if self.click(r, full) {
                self.switch_tab(i);
                self.drag = Some(Drag::Tabs);
            }
        }
        self.switched = (self.switched + self.dt).min(1.0);
        let focused = self.focus == Some(Focus::Search);
        let r = R::new(15.5, 483.5, 144.0, 35.0);
        fill(vg, r, 7.0, rgb(22, 22, 22));
        let ring = if focused || !self.search.is_empty() { self.accent } else { rgb(48, 48, 48) };
        outline(vg, r, 7.0, 0.9, ring);
        let shown = if focused { self.text.clone() } else { self.search.clone() };
        let label = if shown.is_empty() { "Search" } else { shown.as_str() };
        let label_w = text_width(vg, 12.0, Font::Medium, label);
        let (icon_w, gap, pad) = (13.0, 7.0, 10.0);
        let group = icon_w + gap + label_w;
        let start = if group <= r.w - pad * 2.0 { r.x + (r.w - group) / 2.0 } else { r.x + pad };
        let cy = r.y + r.h / 2.0;
        text(vg, start + icon_w / 2.0, cy, 13.0, Font::Icons, rgb(120, 120, 120), ALIGN_CENTER, "\u{f002}");
        let left = start + icon_w + gap;
        let tx = left.min(r.x + r.w - pad - label_w);
        vg.save();
        vg.intersect_scissor(left - 1.0, r.y, r.x + r.w - pad - left + 2.0, r.h);
        let color = if shown.is_empty() { rgb(110, 110, 110) } else { TEXT };
        text(vg, tx, cy, 12.0, Font::Medium, color, ALIGN_LEFT, label);
        if focused && (self.time * 2.0) as i32 % 2 == 0 {
            let x = tx + 0.5 + if shown.is_empty() { 0.0 } else { label_w };
            line(vg, &[[x, cy - 6.5], [x, cy + 6.5]], 1.0, TEXT);
        }
        vg.restore();
        if self.click(r, full) {
            self.focus = Some(Focus::Search);
            self.text = self.search.clone();
        }
        if focused {
            self.follow_search();
        }
        self.footer(vg);
    }

    fn face_texture(&mut self, vg: &mut Frame<'_>, face: [[u8; 3]; 64]) -> Option<i32> {
        let context = vg.context_id();
        if let Some((ctx, image, pixels)) = self.face_image {
            if ctx == context && pixels == face {
                return Some(image);
            }
            if ctx == context {
                vg.delete_image(image);
            }
            self.face_image = None;
        }
        let rgba: Vec<u8> = face.iter().flat_map(|[r, g, b]| [*r, *g, *b, 255]).collect();
        let image = vg.create_image_nearest(8, 8, &rgba)?;
        self.face_image = Some((context, image, face));
        Some(image)
    }

    fn switch_tab(&mut self, i: usize) {
        if i == self.category {
            return;
        }
        self.category = i;
        self.switched = 0.0;
        self.panel = None;
        self.choice = None;
        self.bind_menu = None;
        self.picker = None;
        self.profiles_loaded = false;
    }

    fn footer(&mut self, vg: &mut Frame<'_>) {
        let account = account::account();
        let (x, y, size) = (14.7, 537.3, 45.5);
        match account.face {
            Some(face) => {
                if let Some(image) = self.face_texture(vg, face) {
                    let paint = vg.image_pattern([x, y, size, size], image, 1.0);
                    vg.begin_path();
                    vg.rounded_rect(x, y, size, size, size * 6.0 / 36.0);
                    vg.fill_paint(paint);
                    vg.fill();
                }
            }
            None => {
                fill(vg, R::new(x, y, size, size), size * 6.0 / 36.0, rgb(40, 40, 40));
                let initial: String = account
                    .name
                    .as_deref()
                    .and_then(|n| n.chars().next())
                    .map(|c| c.to_uppercase().collect())
                    .unwrap_or_default();
                text(vg, x + size / 2.0, y + size / 2.0, 20.0, Font::Bold, rgb(150, 150, 150), ALIGN_CENTER, &initial);
            }
        }
        vg.save();
        vg.intersect_scissor(70.0, 530.0, 100.0, 60.0);
        let name = account.name.unwrap_or_else(|| "Player".into());
        text(vg, 72.0, y + size / 2.0 - 8.75, 12.0, Font::Bold, TEXT, ALIGN_LEFT, &name);
        let license = account::license().unwrap_or_else(|| "Slinky".into());
        text(vg, 72.0, y + size / 2.0 + 8.75, 11.0, Font::Medium, rgb(105, 105, 105), ALIGN_LEFT, &license);
        vg.restore();
    }

    fn settings_page(&mut self, vg: &mut Frame<'_>) {
        let Ok(map) = crate::automap::get() else { return };
        let modules: Vec<Module> = GLOBALS
            .iter()
            .filter_map(|name| {
                let m = map.modules.get(*name)?;
                Some(Module { descriptor: map.base + m.descriptor, name: (*name).into(), description: String::new() })
            })
            .collect();
        let scroll = self.ease(107, self.scroll[SETTINGS], 14.0);
        self.scroll_view[SETTINGS] = scroll;
        let y0 = -scroll;
        text(vg, 197.0, 61.0 + y0, 19.0, Font::Bold, rgb(240, 240, 240), ALIGN_LEFT, "Settings");
        let (x, w) = (197.0, 530.0);
        let mut y = 92.0 + y0;
        for module in &modules {
            let d = module.descriptor;
            text(vg, x, y + 15.0, 11.5, Font::Bold, rgb(81, 81, 81), ALIGN_LEFT, &module.name);
            y += 30.0;
            if module.bindable() && data::can_bind() {
                let label = if module.name == "GUI" { "Menu key" } else { "Keybind" };
                text(vg, x, y + 14.25, 13.5, Font::Medium, rgb(160, 160, 160), ALIGN_LEFT, label);
                let waiting = self.focus == Some(Focus::Bind(d));
                let shown = if waiting {
                    "...".to_string()
                } else {
                    match module.keybind() {
                        0 => "None".to_string(),
                        k => {
                            let name = data::key_name(k).to_string();
                            if module.hold() { format!("{name} (Hold)") } else { name }
                        }
                    }
                };
                let menu_open = self.bind_menu.is_some_and(|b| b.0 == d);
                let badge = self.key_badge(vg, x + w, y + 3.0, &shown, waiting || menu_open);
                if self.click(badge, CONTENT) {
                    self.focus = Some(Focus::Bind(d));
                }
                if module.holdable() && self.right_click(badge, CONTENT) {
                    self.bind_menu = Some((d, R::new(badge.x + badge.w - 125.0, badge.y + 28.0, 125.0, 80.0)));
                }
                y += 42.0;
            }
            if module.togglable() {
                let on = module.enabled();
                text(vg, x, y + 12.0, 13.5, Font::Medium, if on { TEXT } else { rgb(127, 127, 127) }, ALIGN_LEFT, "Enabled");
                self.switch(vg, R::new(x + w - 42.0, y + 0.5, 42.0, 23.0), on, id(d, 3));
                if self.click(R::new(x, y, w, 30.0), CONTENT) {
                    data::set_enabled(self.manager, module, !on);
                }
                y += 42.0;
            }
            for item in record::items(d, module.gui_callback()) {
                let h = self.goal(&item);
                if y + h >= CONTENT.y - 4.0 && y <= CONTENT.y + CONTENT.h {
                    vg.save();
                    vg.intersect_scissor(193.0, y - 4.0, 540.0, h + 4.0);
                    let clip = CONTENT.intersect(R::new(x, y, w, h));
                    self.control(vg, &item, x, y, w, clip);
                    vg.restore();
                }
                y += h;
            }
            y += 12.0;
        }
        let content = y + 20.0 + scroll;
        let max = (content - CONTENT.h - CONTENT.y).max(0.0);
        self.scroll[SETTINGS] = self.scroll[SETTINGS].clamp(0.0, max);
        self.scroll_fades(vg, scroll, max);
    }

    fn follow_search(&mut self) {
        let query = squash(&self.text);
        if query.is_empty() || self.category >= PROFILES {
            return;
        }
        let matches = |cat: usize| data::modules(self.manager, cat).iter().any(|m| squash(&m.name).contains(&query));
        if matches(self.category) {
            return;
        }
        if let Some(cat) = (0..PROFILES).find(|&c| matches(c)) {
            self.category = cat;
            self.panel = None;
        }
    }

    fn has_settings(&mut self, module: &Module) -> bool {
        if let Some(&known) = self.settings.get(&module.descriptor) {
            return known;
        }
        let has = !record::items(module.descriptor, module.gui_callback()).is_empty();
        self.settings.insert(module.descriptor, has);
        has
    }

    fn module_list(&mut self, vg: &mut Frame<'_>) {
        let cat = self.category;
        let modules = data::modules(self.manager, cat);
        let query = squash(&self.search);
        let total = modules.len() as f32 * CARD_STEP + 34.0;
        let max = (total - CONTENT.h).max(0.0);
        self.scroll[cat] = self.scroll[cat].clamp(0.0, max);
        let scroll = self.ease(100 + cat as u64, self.scroll[cat], 14.0);
        self.scroll_view[cat] = scroll;
        let enter = (self.switched / 0.12).min(1.0);
        let enter = 1.0 - (1.0 - enter).powi(3);
        let clip = if self.panel.is_some() { R::default() } else { CONTENT };
        for (row, module) in modules.iter().enumerate() {
            let y = 47.0 + row as f32 * CARD_STEP - scroll + 6.0 * (1.0 - enter);
            if y + 73.5 < CONTENT.y || y > CONTENT.y + CONTENT.h {
                continue;
            }
            let r = R::new(197.0, y, 531.0, 73.5);
            let hit = query.is_empty() || squash(&module.name).contains(&query);
            let enabled = module.enabled();
            let togglable = module.togglable();
            let settings = self.has_settings(module);
            vg.save();
            vg.global_alpha(self.alpha * enter * if hit { 1.0 } else { 0.32 });
            shadow(vg, r, 12.0, 9.0, 0.25);
            let hover = self.hover(r, clip);
            let h = self.ease(id(module.descriptor, 1), if hover { 1.0 } else { 0.0 }, 25.0);
            fill(vg, r, 12.0, gray(28.0 + 4.0 * h));
            if !query.is_empty() && hit {
                outline(vg, R::new(197.5, y + 0.5, 530.0, 72.5), 12.0, 1.2, self.accent);
            } else {
                outline(vg, r, 12.0, 0.65, rgba(42, 42, 42, 61));
            }
            let on = self.ease(id(module.descriptor, 2), if enabled { 1.0 } else { 0.0 }, 22.0);
            vg.save();
            vg.intersect_scissor(219.0, y + 5.0, 380.0, 62.0);
            text(vg, 220.0, y + 27.0, 16.8, Font::Medium, mix(TEXT, self.accent, on), ALIGN_LEFT, &module.name);
            text(vg, 220.0, y + 49.5, 13.2, Font::Medium, gray(105.0 + 30.0 * on), ALIGN_LEFT, &module.description);
            vg.restore();
            let gear_x = if togglable { 639.0 } else { 691.0 };
            let key = module.keybind();
            if key != 0 {
                let name = data::key_name(key);
                let bw = text_width(vg, 10.0, Font::Medium, name) + 26.0;
                let right = if settings { gear_x - 19.0 } else if togglable { 650.0 } else { 709.0 };
                let bx = right - bw;
                fill(vg, R::new(bx, y + 26.25, bw, 21.0), 10.5, rgb(26, 26, 26));
                outline(vg, R::new(bx, y + 26.25, bw, 21.0), 10.5, 0.8, rgb(44, 44, 44));
                text(vg, bx + 10.0, y + 36.75, 8.0, Font::Icons, rgb(86, 86, 86), ALIGN_CENTER, "\u{f11c}");
                text(vg, bx + 17.0, y + 36.75, 10.0, Font::Medium, rgb(120, 120, 120), ALIGN_LEFT, name);
            }
            let gear = R::new(gear_x - 16.0, y + 16.75, 32.0, 40.0);
            if settings {
                let lit = self.hover(gear, clip);
                text(vg, gear_x, y + 36.75, 17.0, Font::Icons, if lit { rgb(165, 165, 165) } else { rgb(85, 85, 85) }, ALIGN_CENTER, "\u{f013}");
            }
            let switch_r = R::new(662.0, y + 25.25, 42.0, 23.0);
            if togglable {
                self.switch(vg, switch_r, enabled, id(module.descriptor, 3));
                if self.click(R::new(660.0, y + 12.0, 48.0, 48.0), clip) {
                    data::set_enabled(self.manager, module, !enabled);
                }
            }
            let openable = settings || module.bindable();
            if settings && self.click(gear, clip) {
                self.open(module.descriptor);
            }
            if self.right_click(r, clip) && openable {
                self.open(module.descriptor);
            }
            if self.click(r, clip) {
                if togglable {
                    data::set_enabled(self.manager, module, !enabled);
                } else if openable {
                    self.open(module.descriptor);
                }
            }
            vg.restore();
        }
        self.scroll_fades(vg, scroll, max);
        if max > 0.0 {
            let track = R::new(740.0, 47.0, 3.0, 530.0);
            self.scrollbar(vg, track, total, CONTENT.h, scroll, clip, false);
        }
    }

    fn open(&mut self, descriptor: usize) {
        self.panel = Some(descriptor);
        self.panel_scroll = 0.0;
        self.panel_t = 0.0;
        self.anims.insert(200_000, 0.0);
        self.choice = None;
        self.bind_menu = None;
    }

    fn scroll_fades(&mut self, vg: &mut Frame<'_>, scroll: f32, max: f32) {
        let cat = self.category as u64;
        let top = self.ease(9900 + cat, if scroll > 3.0 { 1.0 } else { 0.0 }, 15.0);
        let bottom = self.ease(9950 + cat, if max > 2.0 && scroll < max - 3.0 { 1.0 } else { 0.0 }, 15.0);
        let shade = |a: f32| rgba(15, 15, 15, (a * 255.0) as u8);
        if top > 0.01 {
            gradient(vg, R::new(CONTENT.x, CONTENT.y, CONTENT.w, 38.0), shade(0.92 * top), shade(0.0));
        }
        if bottom > 0.01 {
            gradient(vg, R::new(CONTENT.x, CONTENT.y + CONTENT.h - 38.0, CONTENT.w, 38.0), shade(0.0), shade(0.92 * bottom));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn scrollbar(&mut self, vg: &mut Frame<'_>, track: R, total: f32, visible: f32, offset: f32, clip: R, panel: bool) {
        if total <= visible {
            return;
        }
        let max = total - visible;
        let thumb = (visible / total * track.h).max(14.0);
        let y = track.y + offset / max * (track.h - thumb);
        fill(vg, track, 1.5, rgb(25, 25, 25));
        fill(vg, R::new(track.x, y, track.w, thumb), 1.5, rgb(90, 90, 90));
        if self.click(R::new(track.x - 4.0, track.y, 11.0, track.h), clip) {
            self.drag = Some(if panel { Drag::PanelBar { track, total } } else { Drag::ListBar { track, total } });
        }
    }

    fn switch(&mut self, vg: &mut Frame<'_>, r: R, on: bool, key: u64) {
        let goal = if on { 1.0 } else { 0.0 };
        let t = self.ease(key, goal, 9.0);
        let bg = self.ease(key ^ 0x5555, goal, 22.0);
        fill(vg, r, r.h / 2.0, mix(rgb(26, 26, 26), self.accent, bg));
        outline(vg, r, r.h / 2.0, 0.8, rgb(42, 42, 42));
        let size = r.h - 8.0;
        let inner = r.w - 1.6;
        let cx = r.x + 0.8 + 3.0 + size / 2.0 + (inner - 6.0 - size) * t;
        let squish = 4.0 * t * (1.0 - t) * 4.5;
        let (kw, kh) = (size + squish * 2.0, size - squish);
        let cy = r.y + r.h / 2.0;
        fill(vg, R::new(cx - kw / 2.0, cy - kh / 2.0, kw, kh), kw.min(kh) / 2.0, gray(58.0 - 34.0 * t));
    }

    fn key_badge(&mut self, vg: &mut Frame<'_>, right: f32, y: f32, label: &str, active: bool) -> R {
        let w = text_width(vg, 11.0, Font::Medium, label) + 39.0;
        let r = R::new(right - w, y, w, 22.5);
        fill(vg, r, 11.25, rgb(19, 19, 19));
        outline(vg, r, 11.25, 0.9, if active { self.accent } else { rgb(42, 42, 42) });
        text(vg, r.x + 14.3, r.y + r.h / 2.0, 10.5, Font::Icons, rgb(86, 86, 86), ALIGN_CENTER, "\u{f11c}");
        let bound = label != "None";
        text(vg, r.x + 27.0, r.y + r.h / 2.0, 11.0, Font::Medium, if bound { rgb(158, 158, 158) } else { rgb(100, 100, 100) }, ALIGN_LEFT, label);
        r
    }

    fn panel(&mut self, vg: &mut Frame<'_>, _out: &mut Output) {
        let opening = self.panel.is_some();
        if opening {
            self.panel_shown = self.panel;
        }
        self.panel_t = if opening { (self.panel_t + self.dt / 0.18).min(1.0) } else { (self.panel_t - self.dt / 0.18).max(0.0) };
        let Some(descriptor) = self.panel_shown else {
            return;
        };
        if self.panel_t <= 0.0 {
            self.panel_shown = None;
            return;
        }
        let k = 1.0 - (1.0 - self.panel_t).powi(3);
        let (cx, cy) = (462.5, 312.5);
        vg.save();
        vg.translate(cx, cy);
        let s = snap_scale(0.88 + 0.12 * k);
        vg.scale(s, s);
        vg.translate(-cx, -cy);
        vg.global_alpha(self.alpha * k);
        let blocked = self.blocked;
        if !opening || self.panel_t < 1.0 {
            self.blocked = true;
        }
        let module = module_at(descriptor);
        shadow(vg, R::new(190.5, 37.5, 544.0, 550.0), 15.0, 24.0, 0.27);
        fill(vg, R::new(190.5, 37.5, 544.0, 550.0), 15.0, DARK);
        text(vg, 215.0, 69.0, 22.5, Font::Bold, TEXT, ALIGN_LEFT, &module.name);
        let full = R::new(0.0, 0.0, W, H);
        let close = R::new(677.0, 53.0, 32.0, 32.0);
        let lit = self.hover(close, full);
        vg.begin_path();
        vg.circle(693.0, 69.0, 16.0);
        vg.fill_color(if lit { rgb(50, 50, 50) } else { rgb(38, 38, 38) });
        vg.fill();
        line(vg, &[[687.0, 63.0], [699.0, 75.0]], 1.2, rgb(143, 143, 143));
        line(vg, &[[699.0, 63.0], [687.0, 75.0]], 1.2, rgb(143, 143, 143));
        if self.click(close, full) {
            self.panel = None;
        }
        line(vg, &[[215.0, 102.0], [709.0, 102.0]], 0.6, rgb(41, 41, 41));

        let items = record::items(descriptor, module.gui_callback());
        let header = if module.togglable() || module.bindable() { 42.0 } else { 0.0 };
        let heights: Vec<f32> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let goal = self.goal(item);
                let key = id(descriptor, 10_000 + item_key(item, i));
                self.ease(key, goal, 16.0)
            })
            .collect();
        let total = heights.iter().sum::<f32>() + header + 15.0;
        let max = (total - 460.0).max(0.0);
        self.panel_scroll = self.panel_scroll.clamp(0.0, max);
        let scroll = self.ease(200_000, self.panel_scroll, 14.0);
        self.panel_scroll_view = scroll;
        let mut y = 120.0 - scroll;
        vg.save();
        vg.intersect_scissor(VIEWPORT.x, VIEWPORT.y, VIEWPORT.w, VIEWPORT.h);
        if header > 0.0 {
            let right = if module.togglable() {
                let on = module.enabled();
                text(vg, 215.0, y + 14.25, 13.5, Font::Medium, if on { TEXT } else { rgb(127, 127, 127) }, ALIGN_LEFT, "Enabled");
                self.switch(vg, R::new(667.0, y + 2.75, 42.0, 23.0), on, id(descriptor, 3));
                if self.click(R::new(667.0, y, 42.0, 32.0), VIEWPORT) {
                    data::set_enabled(self.manager, &module, !on);
                }
                657.0
            } else {
                text(vg, 215.0, y + 14.25, 13.5, Font::Medium, rgb(127, 127, 127), ALIGN_LEFT, "Keybind");
                709.0
            };
            if module.bindable() && data::can_bind() {
                let waiting = self.focus == Some(Focus::Bind(descriptor));
                let label = if waiting {
                    "...".to_string()
                } else {
                    match module.keybind() {
                        0 => "None".to_string(),
                        k => {
                            let name = data::key_name(k).to_string();
                            if module.hold() { format!("{name} (Hold)") } else { name }
                        }
                    }
                };
                let menu_open = self.bind_menu.is_some_and(|b| b.0 == descriptor);
                let badge = self.key_badge(vg, right, y + 3.0, &label, waiting || menu_open);
                if self.click(badge, VIEWPORT) {
                    self.focus = Some(Focus::Bind(descriptor));
                }
                if module.holdable() && self.right_click(badge, VIEWPORT) {
                    self.bind_menu = Some((descriptor, R::new(badge.x + badge.w - 125.0, badge.y + 28.0, 125.0, 80.0)));
                }
            }
            y += header;
        }
        for (i, item) in items.iter().enumerate() {
            let height = heights[i];
            if height < 0.1 {
                continue;
            }
            if y + height >= VIEWPORT.y - 4.0 && y <= VIEWPORT.y + VIEWPORT.h {
                vg.save();
                let full_row = height >= self.goal(item) - 0.5;
                let top = if full_row { 4.0 } else { 0.0 };
                vg.intersect_scissor(213.0, y - top, 498.0, height + top);
                let clip = VIEWPORT.intersect(R::new(215.0, y, 494.0, height));
                self.control(vg, item, 215.0, y, 494.0, clip);
                vg.restore();
            }
            y += height;
        }
        vg.restore();
        if max > 0.0 {
            self.scrollbar(vg, R::new(724.0, 122.0, 3.0, 458.0), total, 460.0, scroll, full, true);
        }
        self.blocked = blocked;
        vg.restore();
    }

    fn goal(&self, item: &Item) -> f32 {
        match item {
            Item::Setting(d, Widget::List) => self.list_heights.get(d).copied().unwrap_or(38.0),
            _ => row_height(item),
        }
    }

    fn list_editor(&mut self, vg: &mut Frame<'_>, d: usize, x: f32, y: f32, w: f32, clip: R) {
        let focused = self.focus == Some(Focus::List(d));
        let label = data::label(d, Widget::List);
        text(vg, x, y + 12.0, 13.5, Font::Medium, rgb(160, 160, 160), ALIGN_LEFT, &label);
        let Some(list) = data::list(d, focused) else {
            self.list_heights.insert(d, 38.0);
            return;
        };
        let count = match list.entries.len() {
            1 => "1 entry".to_string(),
            n => format!("{n} entries"),
        };
        text(vg, x + w, y + 12.0, 11.5, Font::Medium, rgb(90, 90, 90), ALIGN_RIGHT, &count);
        let (chip_h, gap, pad, size) = (24.0, 6.0, 10.0, 11.5);
        let mut cx = x;
        let mut cy = y + 30.0;
        let mut removed = None;
        if list.entries.is_empty() {
            text(vg, x + w / 2.0, cy + chip_h / 2.0, 11.5, Font::Medium, rgb(85, 85, 85), ALIGN_CENTER, "No entries");
        }
        for (i, (name, rgb3)) in list.entries.iter().enumerate() {
            let swatch = if list.colored { 18.0 } else { 0.0 };
            let shown = ellipsize(vg, size, name, w - pad * 2.0 - swatch - 22.0);
            let chip_w = pad + swatch + text_width(vg, size, Font::Medium, &shown) + 8.0 + 10.0 + pad;
            if cx > x && cx + chip_w > x + w {
                cx = x;
                cy += chip_h + gap;
            }
            let chip = R::new(cx, cy, chip_w, chip_h);
            fill(vg, chip, chip_h / 2.0, rgb(26, 26, 26));
            outline(vg, chip, chip_h / 2.0, 0.8, rgb(42, 42, 42));
            let mid = cy + chip_h / 2.0;
            if list.colored {
                let c = rgb3.unwrap_or([1.0; 3]);
                let sw = R::new(cx + pad, mid - 6.0, 12.0, 12.0);
                fill(vg, sw, 3.0, Color::rgb(c));
                outline(vg, sw, 3.0, 0.8, rgba(255, 255, 255, 30));
                if self.click(R::new(cx + pad - 4.0, cy, 20.0, chip_h), clip) {
                    let (hue, sat, val) = to_hsv(c);
                    self.picker = Some(Picker { descriptor: d, entry: Some(i), hue, sat, val });
                }
            }
            text(vg, cx + pad + swatch, mid, size, Font::Medium, rgb(200, 200, 200), ALIGN_LEFT, &shown);
            let close = R::new(cx + chip_w - pad - 14.0, cy, 18.0, chip_h);
            let lit = self.hover(close, clip);
            text(vg, close.x + 7.0, mid, 9.0, Font::Icons, if lit { self.accent } else { rgb(100, 100, 100) }, ALIGN_CENTER, "\u{f00d}");
            if self.click(close, clip) {
                removed = Some(i);
            }
            cx += chip_w + gap;
        }
        cy += chip_h + 10.0;
        if let Some(i) = removed {
            if self.picker.is_some_and(|p| p.descriptor == d) {
                self.picker = None;
            }
            data::list_remove(d, i);
        }
        if data::list_addable(d) {
            let field = R::new(x, cy, w, 28.0);
            let ring = self.ease(id(d, 41), if focused { 1.0 } else { 0.0 }, 20.0);
            fill(vg, field, 6.0, rgb(24, 24, 24));
            outline(vg, field, 6.0, 1.0, mix(rgb(42, 42, 42), faded(self.accent, 0.7), ring));
            let typed = if focused { self.text.clone() } else { String::new() };
            let shown = if !typed.is_empty() {
                typed.clone()
            } else if list.hint.is_empty() {
                "Add entry".to_string()
            } else {
                list.hint.clone()
            };
            let label_w = text_width(vg, 12.0, Font::Medium, &shown);
            let (icon_w, icon_gap, field_pad) = (11.0, 7.0, 10.0);
            let group = icon_w + icon_gap + label_w;
            let start = if group <= field.w - field_pad * 2.0 { field.x + (field.w - group) / 2.0 } else { field.x + field_pad };
            let mid = field.y + field.h / 2.0;
            text(vg, start + icon_w / 2.0, mid, 10.5, Font::Icons, rgb(110, 110, 110), ALIGN_CENTER, "\u{f067}");
            let left = start + icon_w + icon_gap;
            let tx = left.min(field.x + field.w - field_pad - label_w);
            vg.save();
            vg.intersect_scissor(left - 1.0, field.y, field.x + field.w - field_pad - left + 2.0, field.h);
            text(vg, tx, mid, 12.0, Font::Medium, if typed.is_empty() { rgb(100, 100, 100) } else { TEXT }, ALIGN_LEFT, &shown);
            if focused && (self.time * 2.0) as i32 % 2 == 0 {
                let lx = tx + 0.5 + if typed.is_empty() { 0.0 } else { label_w };
                line(vg, &[[lx, mid - 6.5], [lx, mid + 6.5]], 1.0, TEXT);
            }
            vg.restore();
            if self.click(field, clip) && !focused {
                self.commit_text();
                self.focus = Some(Focus::List(d));
                self.text.clear();
            }
            cy += field.h;
            if focused {
                let found = suggestions(&list, &typed);
                let rows: Vec<&String> = found.iter().take(6).collect();
                if !rows.is_empty() {
                    cy += 4.0;
                    let back = R::new(x, cy, w, rows.len() as f32 * 24.0 + 4.0);
                    fill(vg, back, 6.0, rgb(20, 20, 20));
                    outline(vg, back, 6.0, 0.8, rgb(38, 38, 38));
                    for (j, name) in rows.into_iter().enumerate() {
                        let r = R::new(x + 2.0, cy + 2.0 + j as f32 * 24.0, w - 4.0, 24.0);
                        let lit = self.hover(r, clip);
                        if lit {
                            fill(vg, r, 4.0, rgb(30, 30, 30));
                        }
                        text(vg, r.x + r.w / 2.0, r.y + r.h / 2.0, 11.5, Font::Medium, if lit { TEXT } else { rgb(160, 160, 160) }, ALIGN_CENTER, name);
                        if self.click(r, clip) {
                            data::list_add(d, name);
                            self.text.clear();
                        }
                    }
                    cy += back.h;
                } else if !typed.is_empty() {
                    cy += 4.0;
                    text(vg, x + w / 2.0, cy + 12.0, 11.0, Font::Medium, rgb(90, 90, 90), ALIGN_CENTER, "No matches");
                    cy += 24.0;
                }
            }
        }
        self.list_heights.insert(d, (cy - y + 14.0).max(38.0));
    }


    fn control(&mut self, vg: &mut Frame<'_>, item: &Item, x: f32, y: f32, w: f32, clip: R) {
        match *item {
            Item::Section(ref label) => {
                text(vg, x, y + 15.0, 11.5, Font::Bold, rgb(81, 81, 81), ALIGN_LEFT, label);
            }
            Item::Setting(d, Widget::Bool) => {
                let on = data::flag(d);
                let label = data::label(d, Widget::Bool);
                text(vg, x, y + 12.0, 13.5, Font::Medium, if on { TEXT } else { rgb(127, 127, 127) }, ALIGN_LEFT, &label);
                self.switch(vg, R::new(x + w - 42.0, y + 0.5, 42.0, 23.0), on, id(d, 20));
                if self.click(R::new(x, y, w, 30.0), clip) {
                    data::set_flag(d, !on);
                }
            }
            Item::Setting(d, Widget::Color) => {
                let label = data::label(d, Widget::Color);
                text(vg, x, y + 10.0, 13.5, Font::Medium, rgb(160, 160, 160), ALIGN_LEFT, &label);
                let rgb3 = data::color(d).unwrap_or([1.0; 3]);
                let r = R::new(x + w - 19.0, y + 0.5, 19.0, 19.0);
                fill(vg, r, 4.0, Color::rgb(rgb3));
                outline(vg, r, 4.0, 0.8, rgba(255, 255, 255, 30));
                if self.click(R::new(x, y - 2.0, w, 26.0), clip) {
                    let (hue, sat, val) = to_hsv(rgb3);
                    self.picker = Some(Picker { descriptor: d, entry: None, hue, sat, val });
                }
            }
            Item::Setting(d, Widget::Float) => {
                let Some(f) = data::float(d) else { return };
                self.slider(vg, d, Widget::Float, &f.label, x, y, w, clip, f.min, f.max, None, f.value, &data::format(&f.format, f.value * f.mult), f.step);
            }
            Item::Setting(d, Widget::Range) => {
                let Some(r) = data::range(d) else { return };
                let (low_text, high_text) = split_pair(&r.format, r.low * r.mult, r.high * r.mult);
                self.range_slider(vg, d, &r, x, y, w, clip, &low_text, &high_text);
            }
            Item::Setting(d, Widget::Enum) => {
                let Some(c) = data::choice(d) else { return };
                self.choice_box(vg, d, &c, x, y, w, clip);
            }
            Item::Setting(d, Widget::List) => self.list_editor(vg, d, x, y, w, clip),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn slider(&mut self, vg: &mut Frame<'_>, d: usize, widget: Widget, label: &str, x: f32, y: f32, w: f32, clip: R, min: f32, max: f32, _low: Option<f32>, value: f32, shown: &str, step: f32) {
        text(vg, x, y + 8.0, 14.0, Font::Medium, TEXT, ALIGN_LEFT, label);
        let editing = matches!(self.focus, Some(Focus::Number { descriptor, .. }) if descriptor == d);
        let value_w = if editing {
            self.number_field(vg, x + w, y)
        } else {
            text(vg, x + w, y + 8.0, 14.0, Font::Medium, TEXT, ALIGN_RIGHT, shown);
            text_width(vg, 14.0, Font::Medium, shown)
        };
        if self.click(R::new(x + w - value_w - 4.0, y - 2.0, value_w + 6.0, 19.0), clip) {
            self.focus = Some(Focus::Number { descriptor: d, widget, high: false });
            self.text = editable(shown);
        }
        let inset = 24.0;
        let track = R::new(x + inset, y + 22.5, w - inset * 2.0, 10.0);
        let span = max - min;
        let t = if span.abs() > f32::EPSILON { ((value - min) / span).clamp(0.0, 1.0) } else { 0.0 };
        let v = self.ease(id(d, 30), t, 16.0);
        let ry = y + 22.5;
        fill(vg, R::new(track.x, ry - 4.5, track.w, 9.0), 4.5, rgb(20, 20, 20));
        if v > 0.002 {
            fill(vg, R::new(track.x, ry - 4.5, (track.w * v).max(9.0), 9.0), 4.5, self.accent);
            vg.begin_path();
            vg.circle(track.x + track.w * v - 4.5, ry, 3.6);
            vg.fill_color(rgb(24, 24, 24));
            vg.fill();
        }
        let sign = rgb(110, 110, 110);
        line(vg, &[[x + 5.0, ry], [x + 14.5, ry]], 1.5, sign);
        line(vg, &[[x + w - 13.7, ry], [x + w - 4.3, ry]], 1.5, sign);
        line(vg, &[[x + w - 9.0, ry - 4.7], [x + w - 9.0, ry + 4.7]], 1.5, sign);
        let step = if step > 0.0 { step } else { span / 100.0 };
        let unclamped = crate::slinky::settings::UNCLAMPED.load(Ordering::Relaxed);
        let bounded = |v: f32| if unclamped && !(min..=max).contains(&value) { v } else { v.clamp(min.min(max), max.max(min)) };
        if self.click(R::new(x, y + 13.0, 19.0, 22.0), clip) {
            data::set_float(d, bounded(snap_step(value - step, step)));
        }
        if self.click(R::new(x + w - 19.0, y + 13.0, 19.0, 22.0), clip) {
            data::set_float(d, bounded(snap_step(value + step, step)));
        }
        if self.click(R::new(track.x, y + 13.0, track.w, 23.0), clip) {
            self.drag = Some(Drag::Slider { descriptor: d, widget, high: false, track });
            self.dragging(1.0);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn range_slider(&mut self, vg: &mut Frame<'_>, d: usize, r: &data::Range, x: f32, y: f32, w: f32, clip: R, low_text: &str, high_text: &str) {
        text(vg, x, y + 8.0, 14.0, Font::Medium, TEXT, ALIGN_LEFT, &r.label);
        let editing = |high: bool| matches!(self.focus, Some(Focus::Number { descriptor, high: h, .. }) if descriptor == d && h == high);
        let (edit_low, edit_high) = (editing(false), editing(true));
        let high_w = if edit_high {
            self.number_field(vg, x + w, y)
        } else {
            text(vg, x + w, y + 8.0, 14.0, Font::Medium, TEXT, ALIGN_RIGHT, high_text);
            text_width(vg, 14.0, Font::Medium, high_text)
        };
        if self.click(R::new(x + w - high_w - 4.0, y - 2.0, high_w + 6.0, 19.0), clip) {
            self.focus = Some(Focus::Number { descriptor: d, widget: Widget::Range, high: true });
            self.text = editable(high_text);
        }
        let dash = " - ";
        let dw = text_width(vg, 14.0, Font::Medium, dash);
        let dx = x + w - high_w;
        text(vg, dx, y + 8.0, 14.0, Font::Medium, TEXT, ALIGN_RIGHT, dash);
        let lx = dx - dw;
        let low_w = if edit_low {
            self.number_field(vg, lx, y)
        } else {
            text(vg, lx, y + 8.0, 14.0, Font::Medium, TEXT, ALIGN_RIGHT, low_text);
            text_width(vg, 14.0, Font::Medium, low_text)
        };
        if self.click(R::new(lx - low_w - 3.0, y - 2.0, low_w + 4.0, 19.0), clip) {
            self.focus = Some(Focus::Number { descriptor: d, widget: Widget::Range, high: false });
            self.text = editable(low_text);
        }
        let track = R::new(x, y + 22.5, w, 10.0);
        let span = r.max - r.min;
        let norm = |v: f32| if span.abs() > f32::EPSILON { ((v - r.min) / span).clamp(0.0, 1.0) } else { 0.0 };
        let lo = self.ease(id(d, 31), norm(r.low), 16.0);
        let hi = self.ease(id(d, 32), norm(r.high), 16.0);
        let ry = y + 22.5;
        fill(vg, R::new(track.x, ry - 4.5, track.w, 9.0), 4.5, rgb(20, 20, 20));
        fill(vg, R::new(track.x + track.w * lo, ry - 4.5, (track.w * (hi - lo)).max(9.0), 9.0), 4.5, self.accent);
        for (cx, side) in [(track.x + track.w * lo + 4.5, lo), (track.x + track.w * hi - 4.5, hi)] {
            let _ = side;
            vg.begin_path();
            vg.circle(cx, ry, 3.6);
            vg.fill_color(rgb(24, 24, 24));
            vg.fill();
        }
        if self.click(R::new(track.x, y + 13.0, track.w, 23.0), clip) {
            let t = ((self.m[0] - track.x) / track.w).clamp(0.0, 1.0);
            let high = (t - norm(r.high)).abs() < (t - norm(r.low)).abs() || (t > norm(r.high));
            self.drag = Some(Drag::Slider { descriptor: d, widget: Widget::Range, high, track });
            self.dragging(1.0);
        }
    }

    fn number_field(&mut self, vg: &mut Frame<'_>, right: f32, y: f32) -> f32 {
        let entry = self.text.clone();
        let tw = text_width(vg, 14.0, Font::Medium, &entry);
        let fw = (tw + 5.0).max(22.0);
        text(vg, right - 3.0, y + 8.0, 14.0, Font::Medium, self.accent, ALIGN_RIGHT, &entry);
        line(vg, &[[right - fw, y + 15.5], [right, y + 15.5]], 1.0, self.accent);
        if (self.time * 2.0) as i32 % 2 == 0 {
            line(vg, &[[right - 0.5, y + 1.5], [right - 0.5, y + 14.5]], 1.2, self.accent);
        }
        fw
    }

    #[allow(clippy::too_many_arguments)]
    fn choice_box(&mut self, vg: &mut Frame<'_>, d: usize, c: &data::Choice, x: f32, y: f32, w: f32, clip: R) {
        let y = y - 3.0;
        let r = R::new(x, y, w, 36.5);
        let hovered = self.hover(r, clip);
        fill(vg, r, 6.0, if hovered { rgb(24, 24, 24) } else { rgb(20, 20, 20) });
        outline(vg, R::new(x + 0.4, y + 0.4, w - 0.8, 35.7), 6.0, 0.8, rgb(40, 40, 40));
        text(vg, x + 13.0, y + 18.25, 14.0, Font::Medium, rgb(220, 220, 220), ALIGN_LEFT, &c.label);
        let value = c.options.get(c.index).cloned().unwrap_or_else(|| "None".into());
        let room = w - 39.0 - 13.0 - text_width(vg, 14.0, Font::Medium, &c.label) - 24.0;
        let shown = ellipsize(vg, 13.5, &value, room);
        let dim = c.nullable && c.index == 0;
        text(vg, x + w - 39.0, y + 18.25, 13.5, Font::Medium, if dim { rgb(110, 110, 110) } else { rgb(181, 181, 181) }, ALIGN_RIGHT, &shown);
        line(vg, &[[x + w - 31.0, y + 7.0], [x + w - 31.0, y + 29.5]], 0.8, rgb(44, 44, 44));
        let open = self.choice.is_some_and(|ch| ch.0 == d);
        if open {
            self.choice = Some((d, r));
        }
        let turn = self.ease(id(d, 40), if open { 1.0 } else { 0.0 }, 16.0);
        let lit = self.ease(id(d, 41), if open { 1.0 } else { 0.0 }, 25.0);
        let angle = turn * std::f32::consts::PI;
        let (sin, cos) = angle.sin_cos();
        let (cx, cy) = (x + w - 15.5, y + 18.5);
        let points = [[-4.5f32, -2.5f32], [0.0, 2.5], [4.5, -2.5]].map(|[dx, dy]| [cx + dx * cos - dy * sin, cy + dx * sin + dy * cos]);
        line(vg, &points, 1.4, gray(120.0 + 80.0 * lit));
        if self.click(r, clip) {
            self.choice = if open { None } else { Some((d, r)) };
            self.choice_t = 0.0;
        }
    }

    fn choice_rect(&self, anchor: R) -> R {
        let Some((d, _)) = self.choice else {
            return R::default();
        };
        let count = data::choice(d).map_or(0, |c| c.options.len());
        let full = (count as f32 * 30.0 + 6.0).min(H - 60.0);
        let below = anchor.y + anchor.h + 4.5;
        let up = below + full > 594.0 && anchor.y - 4.5 - full >= 30.0;
        let y = if up { anchor.y - 4.5 - full } else { below.min(594.0 - full).max(30.0) };
        R::new(anchor.x, y, anchor.w, full)
    }

    fn choice_popup(&mut self, vg: &mut Frame<'_>) {
        let Some((d, anchor)) = self.choice else { return };
        let Some(c) = data::choice(d) else {
            self.choice = None;
            return;
        };
        let r = self.choice_rect(anchor);
        self.choice_t = (self.choice_t + self.dt / 0.12).min(1.0);
        let k = 1.0 - (1.0 - self.choice_t).powi(3);
        let shown = r.h * k;
        vg.save();
        vg.global_alpha(self.alpha * (k * 3.0).min(1.0));
        vg.intersect_scissor(r.x - 1.0, r.y - 1.0, r.w + 2.0, shown + 2.0);
        shadow(vg, r, 6.0, 8.0, 0.35);
        fill(vg, r, 6.0, rgb(17, 17, 17));
        outline(vg, R::new(r.x + 0.4, r.y + 0.4, r.w - 0.8, r.h - 0.8), 6.0, 0.9, rgb(30, 30, 30));
        let rows = ((r.h - 6.0) / 30.0).floor() as usize;
        let scroll_key = id(d, 50);
        let first = (*self.anims.get(&scroll_key).unwrap_or(&0.0) as usize).min(c.options.len().saturating_sub(rows));
        let first = if c.index >= first + rows { c.index + 1 - rows } else { first.min(c.index) };
        self.anims.insert(scroll_key, first as f32);
        let full = R::new(0.0, 0.0, W, H);
        for (k, label) in c.options.iter().enumerate().skip(first).take(rows) {
            let yy = r.y + 3.0 + (k - first) as f32 * 30.0;
            let row = R::new(r.x + 2.0, yy, r.w - 4.0, 30.0);
            let hovered = self.hover(row, full);
            let lit = self.ease(id(d, 60 + k as u64), if hovered { 1.0 } else { 0.0 }, 25.0);
            if lit > 0.01 {
                fill(vg, row, 5.0, faded(gray(24.0), lit));
            }
            let col = if k == c.index { self.accent } else { gray(105.0 + 95.0 * lit) };
            text(vg, r.x + 13.0, yy + 15.0, 14.0, Font::Medium, col, ALIGN_LEFT, label);
            if self.click(row, full) {
                data::set_choice(d, k);
                self.choice = None;
            }
        }
        vg.restore();
        if self.click(r, full) {}
    }

    fn bind_popup(&mut self, vg: &mut Frame<'_>) {
        let Some((d, r)) = self.bind_menu else { return };
        let module = module_at(d);
        shadow(vg, r, 12.0, 12.0, 0.55);
        fill(vg, r, 12.0, rgb(18, 18, 18));
        outline(vg, r, 12.0, 0.8, rgb(39, 39, 39));
        let full = R::new(0.0, 0.0, W, H);
        for (j, (label, hold, icon)) in [("Toggle", false, "\u{f204}"), ("Hold", true, "\u{f04c}")].into_iter().enumerate() {
            let yy = r.y + 20.0 + j as f32 * 36.0;
            let col = if module.hold() == hold { self.accent } else { rgb(110, 110, 110) };
            text(vg, r.x + 18.0, yy, 13.0, Font::Icons, col, ALIGN_CENTER, icon);
            text(vg, r.x + 43.0, yy, 13.0, Font::Medium, TEXT, ALIGN_LEFT, label);
            if self.click(R::new(r.x + 4.0, yy - 16.0, r.w - 8.0, 32.0), full) {
                data::set_hold(&module, hold);
                self.bind_menu = None;
            }
        }
    }

    fn picker_rect() -> R {
        R::new(225.0, 140.0, 300.0, 300.0)
    }

    fn color_picker(&mut self, vg: &mut Frame<'_>) {
        let Some(p) = self.picker else { return };
        let r = Self::picker_rect();
        let full = R::new(0.0, 0.0, W, H);
        vg.begin_path();
        vg.rounded_rect(0.0, 0.0, W, H, 18.0);
        vg.fill_color(rgba(0, 0, 0, 150));
        vg.fill();
        shadow(vg, r, 15.0, 20.0, 0.7);
        fill(vg, r, 15.0, rgb(18, 18, 18));
        outline(vg, r, 15.0, 0.8, rgb(52, 52, 52));
        line(vg, &[[r.x + 20.0, r.y + 50.0], [r.x + 280.0, r.y + 50.0]], 0.6, rgb(36, 36, 36));
        let title = match p.entry {
            Some(i) => data::list(p.descriptor, false)
                .and_then(|l| l.entries.into_iter().nth(i))
                .map(|(name, _)| name)
                .unwrap_or_default(),
            None => data::label(p.descriptor, Widget::Color),
        };
        text(vg, r.x + 20.0, r.y + 26.0, 12.0, Font::Medium, TEXT, ALIGN_LEFT, &title);
        let close = R::new(r.x + 248.0, r.y + 10.0, 32.0, 32.0);
        vg.begin_path();
        vg.circle(r.x + 264.0, r.y + 26.0, 16.0);
        vg.fill_color(if self.hover(close, full) { rgb(50, 50, 50) } else { rgb(35, 35, 35) });
        vg.fill();
        line(vg, &[[r.x + 259.0, r.y + 21.0], [r.x + 269.0, r.y + 31.0]], 1.2, rgb(143, 143, 143));
        line(vg, &[[r.x + 269.0, r.y + 21.0], [r.x + 259.0, r.y + 31.0]], 1.2, rgb(143, 143, 143));
        if self.click(close, full) {
            self.picker = None;
            return;
        }
        let sv = R::new(r.x + 20.0, r.y + 66.0, 200.0, 200.0);
        let hue_color = Color::rgb(hsv(p.hue, 1.0, 1.0));
        paint_rect(vg, sv, [sv.x, sv.y], [sv.x + sv.w, sv.y], Color::rgba(1.0, 1.0, 1.0, 1.0), hue_color);
        paint_rect(vg, sv, [sv.x, sv.y], [sv.x, sv.y + sv.h], Color::rgba(0.0, 0.0, 0.0, 0.0), Color::rgba(0.0, 0.0, 0.0, 1.0));
        outline(vg, R::new(sv.x + p.sat * sv.w - 3.0, sv.y + (1.0 - p.val) * sv.h - 3.0, 6.0, 6.0), 3.0, 1.0, rgb(255, 255, 255));
        let hr = R::new(r.x + 236.0, r.y + 66.0, 16.0, 200.0);
        for j in 0..6 {
            let seg = R::new(hr.x, hr.y + j as f32 * hr.h / 6.0, hr.w, hr.h / 6.0 + 0.4);
            paint_rect(vg, seg, [seg.x, seg.y], [seg.x, seg.y + hr.h / 6.0], Color::rgb(hsv(j as f32 / 6.0, 1.0, 1.0)), Color::rgb(hsv((j + 1) as f32 / 6.0, 1.0, 1.0)));
        }
        outline(vg, R::new(hr.x - 1.0, hr.y + p.hue * hr.h - 2.0, hr.w + 2.0, 4.0), 0.0, 1.0, rgb(255, 255, 255));
        let current = picker_color(p).unwrap_or(hsv(p.hue, p.sat, p.val));
        fill(vg, R::new(r.x + 20.0, r.y + 276.0 - 4.0, 232.0, 16.0), 4.0, Color::rgb(current));
        let hex = format!(
            "#{:02X}{:02X}{:02X}",
            (current[0] * 255.0).round() as u8,
            (current[1] * 255.0).round() as u8,
            (current[2] * 255.0).round() as u8
        );
        text(vg, r.x + 264.0, r.y + 280.0, 10.0, Font::Medium, rgb(140, 140, 140), ALIGN_CENTER, &hex);
        if self.click(sv, full) {
            self.drag = Some(Drag::Saturation(sv));
            self.dragging(1.0);
        }
        if self.click(hr, full) {
            self.drag = Some(Drag::Hue(hr));
            self.dragging(1.0);
        }
        if self.click(r, full) {}
    }

    fn profiles_page(&mut self, vg: &mut Frame<'_>) {
        if !self.profiles_loaded {
            self.profiles = data::profiles();
            self.profiles_loaded = true;
        }
        let clip = if self.panel.is_some() { R::default() } else { CONTENT };
        let accent = self.accent;
        let rows = self.profiles.len() + 1;
        let content = 174.0 + rows as f32 * 49.0 + 20.0;
        let max = (content - CONTENT.h).max(0.0);
        self.scroll[PROFILES] = self.scroll[PROFILES].clamp(0.0, max);
        let scroll = self.ease(106, self.scroll[PROFILES], 14.0);
        self.scroll_view[PROFILES] = scroll;
        let y0 = -scroll;
        text(vg, 197.0, 60.0 + y0, 19.0, Font::Bold, rgb(235, 235, 235), ALIGN_LEFT, "Profiles");
        let can_load = data::can_load_profiles();
        let can_save = data::can_save_profiles();
        if !can_load {
            text(vg, 197.0, 90.5 + y0, 11.0, Font::Medium, rgb(160, 90, 90), ALIGN_LEFT, "Profile loading is not mapped for this Slinky build.");
        }
        if can_load {
            text(vg, 197.0, 90.5 + y0, 11.0, Font::Medium, rgb(80, 80, 80), ALIGN_LEFT, "New profile");
        }
        let field = R::new(197.5, 101.5 + y0, 529.5, 37.0);
        let focused = self.focus == Some(Focus::ProfileName);
        let ring = self.ease(9800, if focused { 1.0 } else { 0.0 }, 20.0);
        fill(vg, field, 6.0, rgb(24, 24, 24));
        outline(vg, field, 6.0, 1.2, mix(rgb(45, 45, 45), faded(accent, 0.7), ring));
        let entry = if focused { self.text.clone() } else { String::new() };
        vg.save();
        vg.intersect_scissor(field.x + 4.0, field.y, field.w - 36.0, field.h);
        let ty = field.y + field.h / 2.0;
        if entry.is_empty() {
            let hint = if can_save { "Enter profile name..." } else { "Saving profiles is not mapped for this Slinky build" };
            text(vg, 211.0, ty, 13.0, Font::Medium, rgb(85, 85, 85), ALIGN_LEFT, hint);
        } else {
            text(vg, 211.0, ty, 13.0, Font::Medium, rgb(215, 215, 215), ALIGN_LEFT, &entry);
        }
        if focused && (self.time * 2.0) as i32 % 2 == 0 {
            let tx = 211.5 + if entry.is_empty() { 0.0 } else { text_width(vg, 13.0, Font::Medium, &entry) };
            line(vg, &[[tx, ty - 7.0], [tx, ty + 7.0]], 1.0, rgb(215, 215, 215));
        }
        vg.restore();
        if can_save && !entry.is_empty() {
            let ar = R::new(field.x + field.w - 30.0, field.y, 28.0, field.h);
            let col = if self.hover(ar, clip) { accent } else { rgb(100, 100, 100) };
            text(vg, ar.x + ar.w / 2.0, ty, 12.0, Font::Icons, col, ALIGN_CENTER, "\u{f061}");
            if self.click(ar, clip) {
                self.commit_text();
            }
        }
        if can_save && self.click(field, clip) {
            self.focus = Some(Focus::ProfileName);
        }
        text(vg, 197.0, 161.5 + y0, 11.0, Font::Medium, rgb(80, 80, 80), ALIGN_LEFT, "Saved profiles");
        let count = format!("{}", self.profiles.len());
        text(vg, 727.0, 161.5 + y0, 12.0, Font::Medium, rgb(105, 105, 105), ALIGN_RIGHT, &count);
        let names: Vec<Option<String>> = std::iter::once(None).chain(self.profiles.iter().cloned().map(Some)).collect();
        let mut holding = None;
        for (k, name) in names.iter().enumerate() {
            let ry = 174.0 + k as f32 * 49.0 + y0;
            if ry > CONTENT.y + CONTENT.h || ry + 45.0 < CONTENT.y {
                continue;
            }
            let row = R::new(197.5, ry, 529.5, 45.0);
            let hover = self.hover(row, clip);
            let h = self.ease(9810 + k as u64, if hover { 1.0 } else { 0.0 }, 25.0);
            fill(vg, row, 6.0, gray(19.0 + 8.0 * h));
            let label = name.as_deref().unwrap_or("Default");
            let active = self.active_profile.as_deref() == Some(label);
            text(vg, 212.0, ry + 22.5, 13.0, Font::Medium, if active { accent } else { rgb(226, 226, 226) }, ALIGN_LEFT, label);
            if name.is_none() {
                let lx = 212.0 + text_width(vg, 13.0, Font::Medium, label) + 10.0;
                text(vg, lx, ry + 22.5, 11.0, Font::Medium, rgb(90, 90, 90), ALIGN_LEFT, "Resets modules to defaults");
            }
            let load = R::new(624.5 - 13.0, ry + 10.5, 26.0, 24.0);
            if can_load {
                let col = if self.hover(load, clip) { accent } else { rgb(100, 100, 100) };
                text(vg, 624.5, ry + 22.5, 14.0, Font::Icons, col, ALIGN_CENTER, "\u{f063}");
                if self.click(load, clip) {
                    self.load_profile(name.as_deref());
                }
            }
            let Some(name) = name else { continue };
            let save = R::new(663.0 - 13.0, ry + 10.5, 26.0, 24.0);
            if can_save {
                let col = if self.hover(save, clip) { rgb(180, 180, 180) } else { rgb(100, 100, 100) };
                text(vg, 663.0, ry + 22.5, 14.0, Font::Icons, col, ALIGN_CENTER, "\u{f0c7}");
                if self.click(save, clip) {
                    self.save_profile(name);
                }
            }
            let delete = R::new(700.0 - 13.0, ry + 10.5, 26.0, 24.0);
            let over = self.hover(delete, clip);
            text(vg, 700.0, ry + 22.5, 14.0, Font::Icons, if over { rgb(220, 76, 76) } else { rgb(100, 100, 100) }, ALIGN_CENTER, "\u{f2ed}");
            if over && self.down && (self.clicked || self.delete_hold.as_ref().is_some_and(|d| &d.0 == name)) {
                holding = Some(name.clone());
            }
            let progress = self.delete_hold.as_ref().filter(|d| &d.0 == name).map_or(0.0, |d| d.1 / DELETE_HOLD);
            if progress > 0.01 {
                let n = (progress * 48.0).ceil().max(2.0) as usize;
                let pts: Vec<[f32; 2]> = (0..=n)
                    .map(|m| {
                        let a = -std::f32::consts::FRAC_PI_2 + (m as f32 / n as f32) * progress * std::f32::consts::TAU;
                        [700.0 + a.cos() * 12.25, ry + 22.5 + a.sin() * 12.25]
                    })
                    .collect();
                line(vg, &pts, 1.75, rgba(220, 70, 70, ((0.2 + progress * 0.6) * 255.0) as u8));
            }
        }
        match holding {
            Some(name) => {
                self.consumed = true;
                let elapsed = self.delete_hold.as_ref().filter(|d| d.0 == name).map_or(0.0, |d| d.1) + self.dt;
                if elapsed >= DELETE_HOLD {
                    self.delete_hold = None;
                    if data::delete_profile(&name) {
                        notify(&format!("Deleted profile {name}"));
                        if self.active_profile.as_deref() == Some(name.as_str()) {
                            self.active_profile = None;
                        }
                    }
                    self.profiles_loaded = false;
                } else {
                    self.delete_hold = Some((name, elapsed));
                }
            }
            None => self.delete_hold = None,
        }
        self.scroll_fades(vg, scroll, max);
    }

    fn load_profile(&mut self, name: Option<&str>) {
        let label = name.unwrap_or("Default");
        match data::load_profile(self.manager, name) {
            Some(0) => {
                self.active_profile = Some(label.to_string());
                notify(&format!("Loaded profile {label}"));
            }
            Some(code) => notify(&format!("Failed to load profile {label} ({code})")),
            None => {}
        }
        self.settings.clear();
    }

    fn save_profile(&mut self, name: &str) {
        match data::save_profile(self.manager, name, true) {
            Some(0) => {
                self.active_profile = Some(name.to_string());
                notify(&format!("Saved profile {name}"));
            }
            Some(code) => notify(&format!("Failed to save profile {name} ({code})")),
            None => {}
        }
        self.profiles_loaded = false;
    }

    fn create_profile(&mut self, name: &str) {
        let name = name.trim();
        if !data::valid_profile_name(name) {
            notify("Invalid profile name.");
            return;
        }
        match data::save_profile(self.manager, name, false) {
            Some(0) => {
                self.active_profile = Some(name.to_string());
                notify(&format!("Saved profile {name}"));
            }
            Some(data::PROFILE_TAKEN) => notify("Profile name is taken."),
            Some(code) => notify(&format!("Failed to save profile {name} ({code})")),
            None => {}
        }
        self.profiles_loaded = false;
    }
}

fn module_at(descriptor: usize) -> Module {
    Module {
        descriptor,
        name: data::module_name(descriptor),
        description: String::new(),
    }
}

fn notify(message: &str) {
    let _ = crate::slinky::notify_info(message);
}

fn apply_picker(p: Picker) {
    let rgb = hsv(p.hue, p.sat, p.val);
    match p.entry {
        Some(i) => data::set_list_color(p.descriptor, i, rgb),
        None => data::set_color(p.descriptor, rgb),
    }
}

fn picker_color(p: Picker) -> Option<[f32; 3]> {
    match p.entry {
        Some(i) => data::list(p.descriptor, false)?.entries.get(i)?.1,
        None => data::color(p.descriptor),
    }
}

fn suggestions(list: &data::List, typed: &str) -> Vec<String> {
    let typed = typed.trim().to_ascii_lowercase().replace(' ', "_");
    let taken: Vec<String> = list.entries.iter().map(|(n, _)| n.to_ascii_lowercase()).collect();
    let mut found: Vec<(bool, &String)> = list
        .options
        .iter()
        .filter(|o| !taken.contains(&o.to_ascii_lowercase()))
        .filter_map(|o| {
            let lower = o.to_ascii_lowercase();
            lower.contains(&typed).then_some((!lower.starts_with(&typed), o))
        })
        .collect();
    found.sort_by_key(|&(later, o)| (later, o.len()));
    found.into_iter().map(|(_, o)| o.clone()).collect()
}

fn row_height(item: &Item) -> f32 {
    match item {
        Item::Section(_) => 30.0,
        Item::Setting(_, Widget::Bool) => 42.0,
        Item::Setting(_, Widget::Color) => 38.0,
        Item::Setting(_, Widget::Float | Widget::Range | Widget::Enum) => 50.0,
        Item::Setting(_, Widget::List) => 38.0,
    }
}

fn item_key(item: &Item, index: usize) -> u64 {
    match item {
        Item::Section(text) => text.bytes().fold(index as u64 * 31, |h, b| h.wrapping_mul(131).wrapping_add(b as u64)),
        Item::Setting(d, _) => *d as u64,
    }
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase()
}

fn editable(shown: &str) -> String {
    shown
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '.' | ','))
        .collect()
}

fn snap_step(value: f32, step: f32) -> f32 {
    if step > 0.0 {
        (value / step).round() * step
    } else {
        value
    }
}

fn split_pair(format: &str, low: f32, high: f32) -> (String, String) {
    let joined = data::format_pair(format, low, high);
    match joined.split_once(" - ") {
        Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
        None => (data::format("%.2f", low), data::format("%.2f", high)),
    }
}

fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - f * s), v * (1.0 - (1.0 - f) * s));
    match i as i32 % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

fn to_hsv([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= f32::EPSILON {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    let s = if max <= f32::EPSILON { 0.0 } else { d / max };
    (h, s, max)
}

fn fill(vg: &mut Frame<'_>, r: R, radius: f32, color: Color) {
    vg.begin_path();
    vg.rounded_rect(r.x, r.y, r.w, r.h, radius);
    vg.fill_color(color);
    vg.fill();
}

fn outline(vg: &mut Frame<'_>, r: R, radius: f32, width: f32, color: Color) {
    vg.begin_path();
    vg.rounded_rect(r.x, r.y, r.w, r.h, radius);
    vg.stroke_color(color);
    vg.stroke_width(width);
    vg.stroke();
}

fn line(vg: &mut Frame<'_>, points: &[[f32; 2]], width: f32, color: Color) {
    let Some((first, rest)) = points.split_first() else { return };
    vg.begin_path();
    vg.move_to(first[0], first[1]);
    for p in rest {
        vg.line_to(p[0], p[1]);
    }
    vg.stroke_color(color);
    vg.stroke_width(width);
    vg.line_cap(ROUND);
    vg.line_join(ROUND);
    vg.stroke();
    vg.line_cap(BUTT);
    vg.line_join(MITER);
}

fn shadow(vg: &mut Frame<'_>, r: R, radius: f32, feather: f32, alpha: f32) {
    let paint = vg.box_gradient([r.x, r.y + 2.0, r.w, r.h], radius, feather, Color::rgba(0.0, 0.0, 0.0, alpha), Color::rgba(0.0, 0.0, 0.0, 0.0));
    vg.begin_path();
    vg.rect(r.x - feather, r.y - feather, r.w + feather * 2.0, r.h + feather * 2.0 + 2.0);
    vg.rounded_rect(r.x, r.y, r.w, r.h, radius);
    vg.path_winding(2);
    vg.fill_paint(paint);
    vg.fill();
}

fn gradient(vg: &mut Frame<'_>, r: R, top: Color, bottom: Color) {
    paint_rect(vg, r, [r.x, r.y], [r.x, r.y + r.h], top, bottom);
}

fn paint_rect(vg: &mut Frame<'_>, r: R, from: [f32; 2], to: [f32; 2], a: Color, b: Color) {
    let paint = vg.linear_gradient(from, to, a, b);
    vg.begin_path();
    vg.rect(r.x, r.y, r.w, r.h);
    vg.fill_paint(paint);
    vg.fill();
}

#[allow(clippy::too_many_arguments)]
fn text(vg: &mut Frame<'_>, x: f32, y: f32, size: f32, font: Font, color: Color, align: i32, s: &str) {
    if s.is_empty() {
        return;
    }
    vg.font(font);
    vg.font_size(size);
    vg.fill_color(color);
    vg.text_align(align | ALIGN_MIDDLE);
    vg.text(x, y, s);
}

fn text_width(vg: &mut Frame<'_>, size: f32, font: Font, s: &str) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    vg.font(font);
    vg.font_size(size);
    vg.text_align(ALIGN_LEFT | ALIGN_MIDDLE);
    vg.text_advance(s)
}

fn ellipsize(vg: &mut Frame<'_>, size: f32, s: &str, room: f32) -> String {
    if text_width(vg, size, Font::Medium, s) <= room {
        return s.to_string();
    }
    let mut shown: String = s.to_string();
    while !shown.is_empty() && text_width(vg, size, Font::Medium, &format!("{shown}...")) > room {
        shown.pop();
    }
    format!("{}...", shown.trim_end())
}

fn snap_scale(value: f32) -> f32 {
    (value * 100.0).round() / 100.0
}
