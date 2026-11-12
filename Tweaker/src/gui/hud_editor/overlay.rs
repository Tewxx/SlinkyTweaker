use super::{
    arraylist, custom_name,
    element::{self, Element, Rect},
    input::{self, Mouse},
    layout,
    module_edit::{self, ModuleEdit},
    native, timewarp, visibility, Runtime, HUD_ANIM, RUNTIME, VISIBLE,
};
use crate::gui::nanovg::{Color, Font, Frame, ALIGN_CENTER, ALIGN_LEFT, ALIGN_MIDDLE};
use crate::gui::nanovg::{BUTT, MITER, ROUND};
use crate::slinky::mem::{read_f32, read_flag, read_u32};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::{cell::RefCell, ffi::c_void, time::Instant};

const SNAP_DISTANCE: f32 = 8.0;
const GUI_NATIVE: u8 = 0;
const GUI_SKIP: u8 = 1;
const GUI_FILTERED: u8 = 2;
static GUI_MODE: AtomicU8 = AtomicU8::new(GUI_NATIVE);
static FALLBACK: AtomicBool = AtomicBool::new(false);
const SEARCH_TAU: f32 = 0.06;
const CLOSED_GAP: f32 = 0.25;
const OPEN_DURATION: f32 = 0.3;
const OPEN_SLIDE: f32 = 50.0;
pub(super) const GREY_14: f32 = 0.078_431_375;
pub(super) const GREY_40: f32 = 0.250_980_4;
pub(super) const GREY_DD: f32 = 0.866_666_7;
const SCALES: [f32; 4] = [0.75, 1.0, 1.5, 2.0];
const HUD_BUTTON_SIZE: f32 = 36.0;
const HUD_BUTTON_MARGIN: f32 = 16.0;
const TOOLTIP_DURATION: f32 = 0.1;

struct State {
    last: Instant,
    open_amount: f32,
    hud_editing: bool,
    hud_anim: f32,
    tooltip: (Option<usize>, f32),
    modules: ModuleEdit,
    dt: f32,
    dragging: Option<&'static Element>,
    grab: [f32; 2],
    timewarp: timewarp::Timewarp,
    timewarp_on: bool,
}
impl State {
    fn new() -> Self {
        Self {
            last: Instant::now(),
            open_amount: 0.0,
            hud_editing: false,
            hud_anim: 0.0,
            tooltip: (None, 0.0),
            modules: ModuleEdit::default(),
            dt: 0.0,
            dragging: None,
            grab: [0.0; 2],
            timewarp: timewarp::Timewarp::default(),
            timewarp_on: false,
        }
    }
}
thread_local! {static STATE: RefCell<State> = RefCell::new(State::new());}

pub(super) fn grey(value: f32, alpha: f32) -> Color {
    Color::rgba(value, value, value, alpha)
}
fn lerp(a: Color, b: Color, t: f32) -> Color {
    let mix = |x: f32, y: f32| x + (y - x) * t;
    Color::rgba(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b), mix(a.a, b.a))
}
pub(super) fn smoothstep(a: f32) -> f32 {
    a * a * (3.0 - 2.0 * a)
}
pub(super) fn smooth_toward(x: f32, target: f32, t: f32) -> f32 {
    x + (target - x) * t.min(1.0)
}
pub(super) fn ease_open(amount: f32) -> f32 {
    1.0 - (1.0 - amount.clamp(0.0, 1.0)).powi(5)
}
fn fit(value: f32, min: f32, max: f32) -> f32 {
    value.min(max).max(min)
}
pub(super) fn bottom_button_gap(scale: f32) -> f32 {
    16.0f32.max(120.0 / scale)
}

fn snap(position: f32, size: f32, limit: f32, margin: f32, guide: &mut Option<f32>) -> f32 {
    *guide = None;
    let mut best = f32::INFINITY;
    let mut result = position;
    for (target, line) in [
        (margin, margin),
        (limit - size - margin, limit - margin),
        ((limit - size) * 0.5, limit * 0.5),
    ] {
        let distance = (position - target).abs();
        if distance <= SNAP_DISTANCE && distance < best {
            best = distance;
            result = target;
            *guide = Some(line);
        }
    }
    result
}

fn gui_scale(runtime: &Runtime) -> f32 {
    runtime
        .gui_scale
        .and_then(read_u32)
        .map_or(1.0, |i| SCALES[(i as usize).min(SCALES.len() - 1)])
}

pub(super) fn fill_rrect(vg: &mut Frame<'_>, r: Rect, radius: f32, color: Color) {
    vg.begin_path();
    vg.rounded_rect(r.x, r.y, r.w, r.h, radius);
    vg.fill_color(color);
    vg.fill();
}
pub(super) fn stroke_rrect(vg: &mut Frame<'_>, r: Rect, radius: f32, color: Color, width: f32) {
    vg.begin_path();
    vg.rounded_rect(r.x, r.y, r.w, r.h, radius);
    vg.stroke_color(color);
    vg.stroke_width(width);
    vg.stroke();
}
pub(super) fn draw_text(vg: &mut Frame<'_>, r: Rect, center: bool, font: Font, size: f32, color: Color, text: &str) {
    vg.font(font);
    vg.font_size(size);
    vg.fill_color(color);
    vg.text_align(if center { ALIGN_CENTER } else { ALIGN_LEFT } | ALIGN_MIDDLE);
    let x = if center { r.x + r.w * 0.5 } else { r.x };
    vg.text(x, r.y + 1.0 + r.h * 0.5, text);
}
pub(super) fn text_width(vg: &mut Frame<'_>, font: Font, size: f32, text: &str) -> f32 {
    vg.font(font);
    vg.font_size(size);
    vg.text_align(ALIGN_LEFT | ALIGN_MIDDLE);
    vg.text_advance(text)
}
fn squares_icon(vg: &mut Frame<'_>, x: f32, y: f32, size: f32, color: Color) {
    let s = size / 24.0;
    vg.begin_path();
    for (ox, oy) in [(3.75, 3.75), (3.75, 13.5), (13.5, 3.75), (13.5, 13.5)] {
        vg.rounded_rect(x + ox * s, y + oy * s, 6.75 * s, 6.75 * s, 2.25 * s);
    }
    vg.stroke_color(color);
    vg.stroke_width((1.5 * s).max(1.0));
    vg.line_cap(ROUND);
    vg.line_join(ROUND);
    vg.stroke();
    vg.line_cap(BUTT);
    vg.line_join(MITER);
}

pub(super) unsafe extern "system" fn backdrop(event: *mut c_void) {
    let hud_anim = f32::from_bits(HUD_ANIM.load(Ordering::Acquire));
    if event.is_null() || hud_anim <= 0.0 {
        return;
    }
    let size = [
        read_f32(event as usize + 0x28).unwrap_or(0.0),
        read_f32(event as usize + 0x2c).unwrap_or(0.0),
    ];
    if !size.iter().all(|v| v.is_finite() && *v > 0.0 && *v < 65536.0) {
        return;
    }
    let result = crate::gui::nanovg::draw(size[0], size[1], |vg| {
        vg.global_alpha(smoothstep(hud_anim));
        vg.begin_path();
        vg.rect(0.0, 0.0, size[0], size[1]);
        vg.fill_color(grey(0.0, 0.25));
        vg.fill();
        Ok(())
    });
    let _ = result;
}

pub(super) unsafe extern "system" fn ui(event: *mut c_void) {
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        STATE.with(|cell| {
            let Ok(mut state) = cell.try_borrow_mut() else {
                return;
            };
            state.frame(runtime, event);
        })
    }));
    FALLBACK.store(result.is_err(), Ordering::Release);
    if result.is_err() {
        input::route(false, false);
        input::set_typing(false);
        swap_listeners(runtime, false, false, false);
    }
}

pub(super) unsafe extern "system" fn gui_dispatch(event: *mut c_void) {
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    let styled = runtime.module_manager.is_some() && timewarp::active() && !FALLBACK.load(Ordering::Acquire);
    match GUI_MODE.load(Ordering::Acquire) {
        GUI_SKIP => {}
        _ if styled => {}
        GUI_FILTERED => unsafe { visibility::gui_render(event) },
        _ => {
            let native: unsafe extern "system" fn(*mut c_void) = unsafe { std::mem::transmute(runtime.gui_render) };
            unsafe { native(event) }
        }
    }
}

fn swap_listeners(runtime: &Runtime, ours: bool, editing: bool, hiding: bool) {
    let skip = native::skip as *const () as usize;
    let filtered = visibility::gui_render as *const () as usize;
    let mode = match (ours, editing, hiding) {
        (false, ..) | (true, false, true) => GUI_NATIVE,
        (true, true, _) => GUI_SKIP,
        (true, false, false) => GUI_FILTERED,
    };
    GUI_MODE.store(mode, Ordering::Release);
    native::replace(runtime.base, &[runtime.gui_render, skip, filtered], gui_dispatch as *const () as usize);
    if let Some(render) = runtime.mod_overlay_render {
        native::swap(runtime.base, render, arraylist::render as *const () as usize, ours);
    }
}

impl State {
    fn frame(&mut self, runtime: &Runtime, event: *mut c_void) {
        let elapsed = self.last.elapsed().as_secs_f32();
        let dt = elapsed.min(0.05);
        self.dt = dt;
        self.last = Instant::now();
        let ctx = native::ui_context(event);
        let ready = ctx.is_some() && input::install();
        let mouse = input::take();
        let gui_open = ready && read_flag(runtime.gui_enabled) && elapsed < CLOSED_GAP;
        let was_editing = self.hud_editing;
        if !gui_open {
            self.open_amount = 0.0;
            self.hud_editing = false;
            self.hud_anim = 0.0;
            self.dragging = None;
            self.modules.reset();
        } else {
            self.open_amount = (self.open_amount + dt / OPEN_DURATION).min(1.0);
            if self.hud_editing && mouse.escape {
                self.hud_editing = false;
            }
            if self.hud_editing {
                self.hud_anim = smooth_toward(self.hud_anim, 1.0, dt / SEARCH_TAU);
                if 1.0 - self.hud_anim < 0.001 {
                    self.hud_anim = 1.0;
                }
            } else {
                self.hud_anim = 0.0;
            }
        }
        let style = ready && !self.hud_editing && timewarp::active();
        let fading = style && !gui_open && self.timewarp.visible();
        let manager = runtime.module_manager.filter(|_| style && (gui_open || fading));
        if manager.is_none() && (self.timewarp_on || self.timewarp.visible()) {
            self.timewarp.reset();
        }
        self.timewarp_on = manager.is_some() && gui_open;
        let keys = input::take_keys();
        let wheel = input::take_wheel();
        let extra = input::take_extra_buttons();
        let mut button_hot = false;
        let mut typing = false;
        let mut close = false;
        if let (true, Some(ctx), Some(screen)) = (gui_open || fading, ctx, input::client_size()) {
            let scale = gui_scale(runtime);
            let result = unsafe {
                crate::gui::nanovg::draw_native(ctx, |vg| {
                    if let (Some(manager), false) = (manager, gui_open) {
                        let input = timewarp::Input { mouse: Mouse::default(), wheel: 0.0, keys: Vec::new(), chars: Vec::new(), extra: 0 };
                        self.timewarp.frame(vg, input, false, screen, scale, manager, self.dt);
                    } else if let Some(manager) = manager {
                        button_hot = self.draw_buttons(vg, mouse, screen, scale, 0.0, false);
                        let mut pointer = mouse;
                        if button_hot {
                            pointer.left.pressed = false;
                            pointer.right.pressed = false;
                        }
                        let chars = if custom_name::editing() { Vec::new() } else { input::take_typed() };
                        let input = timewarp::Input { mouse: pointer, wheel, keys, chars, extra };
                        let out = self.timewarp.frame(vg, input, true, screen, scale, manager, self.dt);
                        typing = out.typing;
                        close = out.close;
                    } else {
                        let pages = runtime
                            .gui_object
                            .zip(runtime.module_manager)
                            .filter(|_| visibility::ready());
                        button_hot = self.draw(vg, mouse, screen, scale, pages);
                    }
                    Ok(())
                })
            };
            if result.is_err() {
                self.hud_editing = false;
            }
        }
        if close {
            if let Some(manager) = manager {
                timewarp::close(manager);
            }
            self.timewarp_on = false;
            typing = false;
        }
        button_hot |= custom_name::update(mouse, button_hot || self.modules.on || self.timewarp_on);
        input::set_typing(typing || custom_name::editing());
        if was_editing && !self.hud_editing {
            layout::save();
        }
        let skip_native = self.hud_editing || (runtime.module_manager.is_some() && timewarp::active());
        swap_listeners(runtime, ready, skip_native, self.modules.on && !skip_native);
        input::route(self.hud_editing || self.timewarp_on, button_hot);
        VISIBLE.store(self.hud_editing || self.hud_anim > 0.0, Ordering::Release);
        HUD_ANIM.store(self.hud_anim.to_bits(), Ordering::Release);
    }

    fn draw(&mut self, vg: &mut Frame<'_>, mouse: Mouse, screen: [f32; 2], scale: f32, pages: Option<(usize, usize)>) -> bool {
        let hud = smoothstep(self.hud_anim);
        let mut hot = false;
        if let Some((gui, manager)) = pages {
            let accent = Color::rgb(crate::slinky::accent_color());
            hot = self.modules.draw(vg, mouse, gui, manager, screen, scale, self.dt, accent);
        }
        if hud < 1.0 {
            hot |= self.draw_buttons(vg, mouse, screen, scale, hud, pages.is_some());
        }
        if self.hud_anim > 0.0 {
            self.draw_editor(vg, mouse, screen, scale);
        }
        hot
    }

    fn draw_buttons(&mut self, vg: &mut Frame<'_>, mouse: Mouse, screen: [f32; 2], scale: f32, hud: f32, pages: bool) -> bool {
        let open = ease_open(self.open_amount);
        let sh = screen[1] / scale;
        let slide = -OPEN_SLIDE + OPEN_SLIDE * open;
        let size = HUD_BUTTON_SIZE;
        let button = |i: usize| Rect {
            x: HUD_BUTTON_MARGIN + i as f32 * (size + 8.0),
            y: sh - size - HUD_BUTTON_MARGIN,
            w: size,
            h: size,
        };
        let count = if pages { 2 } else { 1 };
        let cursor = (!self.hud_editing && mouse.inside).then(|| [mouse.x / scale, mouse.y / scale - slide]);
        let hovered = (0..count).find(|&i| cursor.is_some_and(|[x, y]| button(i).contains(x, y)));
        match hovered {
            Some(i) if Some(i) == self.tooltip.0 => {
                self.tooltip.1 = (self.tooltip.1 + self.dt / TOOLTIP_DURATION).min(1.0);
            }
            _ => self.tooltip = (hovered, 0.0),
        }
        let accent = Color::rgb(crate::slinky::accent_color());
        vg.save();
        vg.scale(scale, scale);
        vg.global_alpha(open * (1.0 - hud));
        vg.translate(0.0, slide);
        for i in 0..count {
            let pill = button(i);
            let lit = hovered == Some(i) || (i == 1 && self.modules.on);
            fill_rrect(vg, pill, size * 0.5, grey(if lit { GREY_40 } else { GREY_14 }, 1.0));
            if i == 0 {
                squares_icon(vg, pill.x + 8.0, pill.y + 8.0, 20.0, accent);
            } else {
                module_edit::pencil_icon(vg, pill.x + 8.0, pill.y + 8.0, 20.0, accent);
            }
        }
        let tooltip = match hovered {
            Some(0) => Some("Edit HUD"),
            Some(1) => Some(if self.modules.on { "Done hiding modules" } else { "Hide modules" }),
            _ => None,
        };
        if let (Some(i), Some(label)) = (hovered, tooltip) {
            let pill = button(i);
            let s = smoothstep(self.tooltip.1);
            let w = text_width(vg, Font::Medium, 14.0, label);
            vg.save();
            vg.translate(pill.x + size * 0.5, pill.y);
            vg.global_alpha(open * (1.0 - hud) * s);
            vg.scale(s, s);
            let tip = Rect { x: w * -0.5 - 5.0, y: -28.0, w: w + 10.0, h: 20.0 };
            fill_rrect(vg, tip, 5.0, grey(GREY_40, 1.0));
            draw_text(vg, tip, true, Font::Medium, 14.0, grey(GREY_DD, 1.0), label);
            vg.restore();
        }
        vg.restore();
        if mouse.left.pressed {
            match hovered {
                Some(0) => {
                    self.hud_editing = true;
                    self.modules.reset();
                }
                Some(1) => self.modules.on = !self.modules.on,
                _ => {}
            }
        }
        hovered.is_some()
    }

    fn draw_editor(&mut self, vg: &mut Frame<'_>, mouse: Mouse, [sw, sh]: [f32; 2], scale: f32) {
        let targets: Vec<(&'static Element, Rect)> = element::registered()
            .into_iter()
            .filter_map(|e| e.take_drawn().map(|r| (e, r)))
            .collect();
        let active = self.hud_editing;
        let (mx, my) = (mouse.x, mouse.y);
        let hovered = (active && mouse.inside)
            .then(|| {
                targets.iter().rposition(|(_, r)| r.inflate(4.0).contains(mx, my))
            })
            .flatten();
        let mut consumed = false;
        if let Some(i) = hovered {
            let (element, r) = targets[i];
            if mouse.left.pressed {
                self.dragging = Some(element);
                self.grab = [mx - r.x, my - r.y];
                consumed = true;
            }
            if mouse.right.pressed {
                element.reset();
            }
        }
        if !active || !mouse.left.down {
            self.dragging = None;
        }
        let dragged = self
            .dragging
            .and_then(|d| targets.iter().position(|(e, _)| std::ptr::eq(*e, d)));
        if dragged.is_none() {
            self.dragging = None;
        }
        let mut guides = [None, None];
        if let Some(i) = dragged {
            let (element, r) = targets[i];
            let m = element.margin();
            let x = fit(mx - self.grab[0], m, (sw - r.w - m).max(m));
            let y = fit(my - self.grab[1], m, (sh - r.h - m).max(m));
            let placed = Rect {
                x: snap(x, r.w, sw, m, &mut guides[0]),
                y: snap(y, r.h, sh, m, &mut guides[1]),
                ..r
            };
            element.place(placed, [sw, sh]);
        }

        let accent = Color::rgb(crate::slinky::accent_color());
        vg.save();
        vg.global_alpha(smoothstep(self.hud_anim));
        let guide = Color { a: 0.8, ..accent };
        if let Some(x) = guides[0] {
            fill_rrect(vg, Rect { x: x - 0.5, y: 0.0, w: 1.0, h: sh }, 0.0, guide);
        }
        if let Some(y) = guides[1] {
            fill_rrect(vg, Rect { x: 0.0, y: y - 0.5, w: sw, h: 1.0 }, 0.0, guide);
        }
        for (i, (_, r)) in targets.iter().enumerate() {
            let lit = hovered == Some(i) || dragged == Some(i);
            let outline = r.inflate(3.0);
            if lit {
                fill_rrect(vg, outline, 6.0, grey(1.0, 0.06));
            }
            let color = if dragged == Some(i) {
                accent
            } else {
                grey(1.0, if lit { 0.8 } else { 0.4 })
            };
            stroke_rrect(vg, outline, 6.0, color, 1.5);
        }

        vg.scale(scale, scale);
        let (uw, uh) = (sw / scale, sh / scale);
        let (gx, gy) = (mx / scale, my / scale);
        let chip_color = grey(GREY_14, 0.9);
        let text_color = grey(GREY_DD, 1.0);
        for (element, r) in &targets {
            let top = r.y / scale;
            let label_y = if top > 26.0 { top - 24.0 } else { (r.y + r.h) / scale + 6.0 };
            let w = text_width(vg, Font::Bold, 12.0, element.label) + 16.0;
            let x = fit((r.x + r.w * 0.5) / scale - w * 0.5, 4.0, uw - w - 4.0);
            let chip = Rect { x, y: label_y, w, h: 18.0 };
            fill_rrect(vg, chip, 9.0, chip_color);
            draw_text(vg, chip, true, Font::Bold, 12.0, text_color, element.label);
        }

        let (done_w, done_h) = (110.0, 36.0);
        let done = Rect {
            x: (uw - done_w) * 0.5,
            y: uh - done_h - bottom_button_gap(scale),
            w: done_w,
            h: done_h,
        };
        let done_hovered = active && mouse.inside && done.contains(gx, gy);
        let fill = if done_hovered {
            lerp(accent, grey(1.0, 1.0), 0.15)
        } else {
            accent
        };
        fill_rrect(vg, done, done_h * 0.5, fill);
        draw_text(vg, done, true, Font::Bold, 15.0, grey(1.0, 1.0), "Done");
        if done_hovered && mouse.left.pressed && !consumed {
            self.hud_editing = false;
            self.dragging = None;
        }
        vg.restore();
    }
}
