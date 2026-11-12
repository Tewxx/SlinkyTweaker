use super::element::Rect;
use super::input::{self, Mouse};
use super::overlay::{draw_text, grey, stroke_rrect, text_width, GREY_40, GREY_DD};
use crate::gui::nanovg::{self, Font};
use crate::slinky::mem::read_u64;
use crate::slinky::registration::{write_ptr, SuspendedThreads};
use crate::slinky::settings as native_settings;
use crate::{automap, Error, Result};
use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const MODULE: &str = "Notifications";
const DEFAULT: &str = "SLINKY";
const MAX_LENGTH: usize = 20;
const ROW_H: f32 = 30.0;
const GREY_50: f32 = 0.313_725_5;

type GuiFn = unsafe extern "system" fn(*mut c_void, *mut c_void);
type NextFn = unsafe extern "system" fn(*mut [f32; 4], *mut c_void, f32) -> *mut [f32; 4];

struct Additions {
    gui: GuiFn,
    next: NextFn,
}
static ADDITIONS: OnceLock<Additions> = OnceLock::new();

struct State {
    name: String,
    editing: Option<String>,
    drawn: Option<Rect>,
    hovered: bool,
}
static STATE: Mutex<State> = Mutex::new(State {
    name: String::new(),
    editing: None,
    drawn: None,
    hovered: false,
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn shown(name: &str) -> &str {
    if name.is_empty() {
        DEFAULT
    } else {
        name
    }
}

pub(super) fn name() -> String {
    shown(&state().name).to_string()
}

fn save(name: &str) {
    let Some(path) = super::visibility::slinky_file("notification_name") else {
        return;
    };
    let temporary = path.with_extension("tmp");
    let _ = std::fs::write(&temporary, name).and_then(|()| std::fs::rename(&temporary, &path));
}

pub(super) fn add_setting() -> Result<()> {
    if let Some(saved) = super::visibility::slinky_file("notification_name").and_then(|p| std::fs::read_to_string(p).ok()) {
        state().name = saved.trim().to_string();
    }
    let map = automap::get()?;
    let module = map
        .modules
        .get(MODULE)
        .ok_or_else(|| Error::Jvm("Notifications is not mapped".into()))?;
    let next = map.address("layout_next")?;
    let descriptor = map.base + module.descriptor;
    let (gui_slot, _) = automap::callback_slots()?;
    let gui = read_u64(descriptor + gui_slot)
        .map(|v| v as usize)
        .filter(|&v| v != 0)
        .ok_or_else(|| Error::Jvm("Notifications has no settings callback".into()))?;
    ADDITIONS
        .set(Additions {
            gui: unsafe { std::mem::transmute::<usize, GuiFn>(gui) },
            next: unsafe { std::mem::transmute::<usize, NextFn>(next) },
        })
        .map_err(|_| Error::Jvm("Notifications additions exist".into()))?;
    super::timewarp::register(descriptor, Some(gui), || super::timewarp::Extras { before: None, items: Vec::new() });
    let _threads = SuspendedThreads::suspend_others()?;
    write_ptr(descriptor + gui_slot, panel as *const () as usize, "Notifications GUI")
}

unsafe extern "system" fn panel(first: *mut c_void, second: *mut c_void) {
    let Some(additions) = ADDITIONS.get() else {
        return;
    };
    unsafe {
        (additions.gui)(first, second);
        native_settings::section(c"Custom name", first, second);
        let mut row = [0.0; 4];
        (additions.next)(&mut row, first, ROW_H);
        let [x, y, w, h] = row;
        draw_box(Rect { x, y, w, h }, second as usize);
    }
}

unsafe fn draw_box(row: Rect, gui: usize) {
    let Some(ctx) = read_u64(gui).and_then(|ctx| automap::ui_context::resolve([ctx as usize; 2])) else {
        return;
    };
    let mut state = state();
    let result = unsafe {
        nanovg::draw_in_place(ctx, |vg| {
            let lit = state.hovered || state.editing.is_some();
            stroke_rrect(vg, row, 8.0, grey(if lit { GREY_50 } else { GREY_40 }, 1.0), 2.0);
            let inner = Rect { x: row.x + 4.0, y: row.y + 4.0, w: row.w - 8.0, h: row.h - 8.0 };
            let text = state.editing.as_deref().unwrap_or_else(|| shown(&state.name));
            draw_text(vg, inner, false, Font::Medium, 15.0, grey(GREY_DD, 1.0), text);
            if state.editing.is_some() {
                static START: OnceLock<Instant> = OnceLock::new();
                let now = START.get_or_init(Instant::now).elapsed().as_secs_f32();
                let x = inner.x + text_width(vg, Font::Medium, 15.0, text) + 1.0;
                vg.begin_path();
                vg.move_to(x, inner.y);
                vg.line_to(x, inner.y + inner.h);
                vg.stroke_color(grey(GREY_DD, 0.5 + 0.5 * (8.0 * now).sin()));
                vg.stroke_width(1.0);
                vg.stroke();
            }
            let [a, b, c, d, e, f] = vg.current_transform();
            let drawn = Rect {
                x: a * row.x + c * row.y + e,
                y: b * row.x + d * row.y + f,
                w: a * row.w,
                h: d * row.h,
            };
            state.drawn = Some(drawn);
            Ok(())
        })
    };
    let _ = result;
}

pub(super) fn update(mouse: Mouse, blocked: bool) -> bool {
    let mut state = state();
    let drawn = state.drawn.take();
    let hovered = !blocked && mouse.inside && drawn.is_some_and(|r| r.contains(mouse.x, mouse.y));
    state.hovered = hovered;
    let typed = if state.editing.is_some() { input::take_typed() } else { Vec::new() };
    if let Some(mut text) = state.editing.take() {
        let mut keep = drawn.is_none() || (mouse.left.pressed && !hovered);
        let mut cancel = false;
        for c in typed {
            match c {
                '\r' => keep = true,
                '\u{1b}' => cancel = true,
                '\u{8}' => {
                    text.pop();
                }
                '\u{7f}' => text.clear(),
                c if !c.is_control() && text.chars().count() < MAX_LENGTH => text.push(c),
                _ => {}
            }
        }
        if !keep && !cancel {
            state.editing = Some(text);
        } else if !cancel && text.trim() != state.name {
            let name = text.trim().to_string();
            save(&name);
            state.name = name;
        }
    } else if hovered && mouse.left.pressed {
        state.editing = Some(shown(&state.name).to_string());
    }
    hovered
}

pub(super) fn editing() -> bool {
    state().editing.is_some()
}
