mod backdrop;
use super::element::Element;
use super::{input, native, values, RUNTIME};
use crate::gui::nanovg::{Color, Font, Frame, ALIGN_LEFT, ALIGN_TOP};
use crate::slinky::mem::{read_cstr, read_memory, read_u64};
use crate::slinky::registration::{catalog_modules, write_ptr, SuspendedThreads};
use crate::slinky::settings::{self as native_settings, Setting, Spec};
use crate::{Error, Result};
use std::sync::OnceLock;
use std::{cell::RefCell, collections::HashMap, ffi::c_void, time::Instant};

pub(super) static ELEMENT: Element = Element::corner("mod_overlay", "Mod Overlay", [1.0, 0.0], 0.0);

const ANIMATION_SPEED: f32 = 5.0;
const DETAIL_GAP: f32 = 5.0;
const OVERLAY: &str = "Mod Overlay";
const FLAG_UNLISTED: u32 = 4;
const CATEGORY_KEYS: [&str; 5] = ["show_combat", "show_move", "show_visual", "show_block", "show_utility"];
const GREY_99: f32 = 0.6;

type GuiFn = unsafe extern "system" fn(*mut c_void, *mut c_void);
type SyncFn = unsafe extern "system" fn(*mut c_void);

struct Additions {
    settings: [Setting; 5],
    gui: GuiFn,
    sync: SyncFn,
}
static ADDITIONS: OnceLock<Additions> = OnceLock::new();
const GRADIENT: usize = 0;
const GRADIENT_COLOR: usize = 1;
const BLUR: usize = 2;
const BLOOM: usize = 3;
const OPACITY: usize = 4;

pub(super) fn add_settings(base: usize) -> Result<()> {
    let map = crate::automap::get()?;
    let module = map
        .modules
        .get(OVERLAY)
        .ok_or_else(|| Error::Jvm("Mod Overlay is not mapped".into()))?;
    let descriptor = map.base + module.descriptor;
    let callback = |offset| {
        read_u64(descriptor + offset)
            .map(|v| v as usize)
            .filter(|&v| v != 0)
            .ok_or_else(|| Error::Jvm("Mod Overlay has no settings callbacks".into()))
    };
    let (gui_slot, sync_slot) = crate::automap::callback_slots()?;
    let (gui, sync) = (callback(gui_slot)?, callback(sync_slot)?);
    let settings = native_settings::create(
        base,
        [
            Spec::boolean("Gradient", "gradient", false),
            Spec::color("Gradient color", "gradient_color", [1.0, 123.0 / 255.0, 0.0]),
            Spec::boolean("Blur", "blur", false),
            Spec::boolean("Bloom", "bloom", false),
            Spec::float("Background opacity", "background_opacity", 0.5, 0.0, 1.0, "%.0f%%")
                .step(0.01)
                .times(100.0),
        ],
    )?;
    ADDITIONS
        .set(Additions {
            settings,
            gui: unsafe { std::mem::transmute::<usize, GuiFn>(gui) },
            sync: unsafe { std::mem::transmute::<usize, SyncFn>(sync) },
        })
        .map_err(|_| Error::Jvm("Mod Overlay additions exist".into()))?;
    super::timewarp::register(descriptor, Some(gui), timewarp_extras);
    let _threads = SuspendedThreads::suspend_others()?;
    write_ptr(descriptor + gui_slot, additions_gui as *const () as usize, "Mod Overlay GUI")?;
    write_ptr(descriptor + sync_slot, additions_sync as *const () as usize, "Mod Overlay sync")
}

unsafe extern "system" fn additions_gui(gui: *mut c_void, layout: *mut c_void) {
    let Some(g) = ADDITIONS.get() else {
        return;
    };
    unsafe {
        (g.gui)(gui, layout);
        native_settings::gui(&g.settings[..GRADIENT_COLOR], gui, layout);
        if g.settings[GRADIENT].flag() {
            native_settings::gui(&g.settings[GRADIENT_COLOR..BLUR], gui, layout);
        }
        native_settings::gui(&g.settings[BLUR..], gui, layout);
    }
}
unsafe extern "system" fn additions_sync(ctx: *mut c_void) {
    let Some(g) = ADDITIONS.get() else {
        return;
    };
    unsafe {
        (g.sync)(ctx);
        native_settings::sync(&g.settings, ctx);
    }
}

fn gradient_color() -> Option<[f32; 3]> {
    let g = ADDITIONS.get()?;
    g.settings[GRADIENT].flag().then(|| g.settings[GRADIENT_COLOR].color()).flatten()
}

fn background_opacity() -> f32 {
    ADDITIONS
        .get()
        .map_or(0.5, |g| g.settings[OPACITY].number(0.5))
        .clamp(0.0, 1.0)
}

fn addition(i: usize) -> bool {
    ADDITIONS.get().is_none_or(|g| g.settings[i].flag())
}

fn row_color(first: [f32; 3], second: [f32; 3], clock_ms: i64, row: usize) -> [f32; 3] {
    let angle = (clock_ms / 6 - row as i64 * 20).rem_euclid(360) as f32;
    let angle = if angle >= 180.0 { 360.0 - angle } else { angle } * 2.0;
    let t = (angle / 360.0).clamp(0.0, 1.0);
    [0, 1, 2].map(|i| first[i] + (second[i] - first[i]) * t)
}

fn clock_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

struct Module {
    name: String,
    category: u32,
    enabled: bool,
    flags: u32,
    keybind: i32,
}

struct Item {
    descriptor: usize,
    name: String,
    detail: String,
    animation: f32,
    width: f32,
    height: f32,
    shown: bool,
}
impl Item {
    fn preview(name: &str) -> Self {
        Self {
            descriptor: 0,
            name: name.into(),
            detail: String::new(),
            animation: 1.0,
            width: 0.0,
            height: 0.0,
            shown: true,
        }
    }
}

struct State {
    last: Instant,
    items: Vec<Item>,
    names: HashMap<usize, String>,
    glow: backdrop::Glow,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        last: Instant::now(),
        items: Vec::new(),
        names: HashMap::new(),
        glow: backdrop::Glow::default(),
    });
}

fn smoothstep(a: f32) -> f32 {
    a * a * (3.0 - 2.0 * a)
}

impl State {
    fn module(&mut self, descriptor: usize) -> Option<Module> {
        let header = read_memory(descriptor, 0x20)?;
        let u32_at = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
        let name = match self.names.get(&descriptor) {
            Some(name) => name.clone(),
            None => {
                let pointer = u64::from_le_bytes(header[..8].try_into().unwrap()) as usize;
                let name = read_cstr(pointer, 64)?;
                self.names.insert(descriptor, name.clone());
                name
            }
        };
        Some(Module {
            name,
            category: u32_at(0x10),
            enabled: header[0x14] == 1,
            flags: u32_at(0x18),
            keybind: u32_at(0x1c) as i32,
        })
    }

    fn update(&mut self, base: usize, dt: f32) {
        if !values::enabled(OVERLAY) {
            self.items.clear();
            return;
        }
        for item in &mut self.items {
            item.shown = false;
        }
        let require_bound = values::flag(OVERLAY, "require_bound", false);
        for descriptor in catalog_modules(base).unwrap_or_default() {
            let Some(module) = self.module(descriptor) else {
                continue;
            };
            let listed = module.enabled
                && module.flags & FLAG_UNLISTED == 0
                && !(require_bound && module.keybind == 0)
                && CATEGORY_KEYS
                    .get(module.category as usize)
                    .is_some_and(|key| values::flag(OVERLAY, key, true));
            if !listed {
                continue;
            }
            let index = match self.items.iter().position(|i| i.descriptor == descriptor) {
                Some(index) => index,
                None => {
                    self.items.push(Item {
                        descriptor,
                        animation: 0.0,
                        ..Item::preview(&module.name)
                    });
                    self.items.len() - 1
                }
            };
            let item = &mut self.items[index];
            item.shown = true;
            item.name = module.name;
        }
        let lowercase = values::flag(OVERLAY, "lowercase", true);
        let show_details = values::flag(OVERLAY, "show_details", true);
        for item in &mut self.items {
            let delta = dt * ANIMATION_SPEED * if item.shown { 1.0 } else { -1.0 };
            item.animation = (item.animation + delta).clamp(0.0, 1.0);
            item.detail = if show_details { detail(&item.name) } else { String::new() };
            if lowercase {
                item.name = item.name.to_lowercase();
                item.detail = item.detail.to_lowercase();
            }
        }
        self.items.retain(|i| i.shown || i.animation > 0.0);
    }
}

fn detail(name: &str) -> String {
    let original = [
        "Aim Assist", "Auto Clicker", "Backtrack", "Blink", "Criticals", "Fake Lag", "Fast Mine",
        "Fast Place", "Hit Select", "Hitboxes", "Inv Walk", "Keep Sprint", "Knockback Delay",
        "Knockback Displace", "Lag Range", "No Hit Delay", "No Item Release", "Player ESP",
        "Pointers", "Reach", "Sprint Reset", "Timer", "Timer Range", "Velocity",
    ]
    .into_iter()
    .find(|n| n.eq_ignore_ascii_case(name));
    let Some(module) = original else {
        return String::new();
    };
    let number = |key, format: fn(f32) -> String| {
        values::number(module, key).map(format).unwrap_or_default()
    };
    let label = |key| values::option(module, key).unwrap_or_default();
    match module {
        "Aim Assist" | "Criticals" | "Fake Lag" | "Hit Select" | "Inv Walk" | "No Hit Delay"
        | "No Item Release" | "Player ESP" | "Sprint Reset" | "Timer Range" => label("mode"),
        "Auto Clicker" => label("pattern"),
        "Blink" | "Knockback Displace" => label("direction"),
        "Pointers" => label("style"),
        "Backtrack" | "Lag Range" => number("delay", |v| format!("{v:.0}ms")),
        "Knockback Delay" => number("max_delay", |v| format!("{v:.0}ms")),
        "Fast Mine" | "Timer" => number("speed", |v| format!("{v:.2}x")),
        "Fast Place" => number("max_cps", |v| format!("{v:.1}")),
        "Hitboxes" => number("expand", |v| format!("{v:.2}")),
        "Reach" => number("distance", |v| format!("{v:.2}")),
        "Keep Sprint" => number("motion_mult", |v| format!("{:.0}%", v * 100.0)),
        "Velocity" => match label("mode") {
            mode if mode != "Regular" => mode,
            _ => number("reduce_amount", |v| format!("{:.0}%", v * 100.0)),
        },
        _ => String::new(),
    }
}

pub(super) unsafe extern "system" fn capture(event: *mut c_void) {
    if event.is_null() || !addition(BLUR) {
        return;
    }
    let size = [0x28, 0x2c].map(|at| crate::slinky::mem::read_f32(event as usize + at).unwrap_or(0.0));
    if !size.iter().all(|v| v.is_finite() && *v > 0.0 && *v < 65536.0) {
        return;
    }
    let _ = std::panic::catch_unwind(|| backdrop::capture(size));
}

pub(super) unsafe extern "system" fn render(event: *mut c_void) {
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (Some(ctx), Some(screen)) = (native::ui_context(event), input::client_size()) else {
            return Ok(());
        };
        STATE.with(|cell| {
            let Ok(mut state) = cell.try_borrow_mut() else {
                return Ok(());
            };
            let dt = state.last.elapsed().as_secs_f32().min(0.05);
            state.last = Instant::now();
            state.update(runtime.base, dt);
            let (blur, bloom) = (addition(BLUR), addition(BLOOM));
            let fill = Color::rgba(0.0, 0.0, 0.0, background_opacity());
            let mut cards = Vec::new();
            unsafe {
                crate::gui::nanovg::draw_native(ctx, |vg| {
                    cards = draw(vg, &mut state.items, screen, fill)?;
                    Ok(())
                })?
            };
            backdrop::draw(&cards, screen, fill, blur, bloom, &mut state.glow)
        })
    }));
}

fn draw(vg: &mut Frame<'_>, items: &mut Vec<Item>, [sw, sh]: [f32; 2], fill: Color) -> Result<Vec<backdrop::Card>> {
    let mut preview: Vec<Item>;
    let mut alpha = 1.0;
    let rows = if !values::enabled(OVERLAY) || items.is_empty() {
        if !super::editing() {
            return Ok(Vec::new());
        }
        let lowercase = values::flag(OVERLAY, "lowercase", true);
        preview = ["Aim Assist", "Velocity", "Sprint"]
            .into_iter()
            .map(|n| Item::preview(&if lowercase { n.to_lowercase() } else { n.to_string() }))
            .collect();
        alpha = 0.6;
        &mut preview
    } else {
        items
    };

    let text_size = values::number(OVERLAY, "text_size").unwrap_or(16.0);
    let padding_x = values::number(OVERLAY, "padding_x").unwrap_or(3.0);
    let padding_y = values::number(OVERLAY, "padding_y").unwrap_or(1.0);
    let rounding = values::number(OVERLAY, "bg_rounding").unwrap_or(6.0);
    let name_color = values::color(OVERLAY, "name_color")
        .unwrap_or_else(crate::slinky::accent_color);
    let details_color = values::color(OVERLAY, "details_color").unwrap_or([GREY_99; 3]);
    let gradient = gradient_color();
    let clock = clock_ms();

    vg.font(Font::Medium);
    vg.font_size(text_size);
    vg.text_align(ALIGN_LEFT | ALIGN_TOP);
    for item in rows.iter_mut() {
        let b = vg.text_bounds(0.0, 0.0, &item.name);
        (item.width, item.height) = (b[2], b[3]);
        if !item.detail.is_empty() {
            let d = vg.text_bounds(0.0, 0.0, &item.detail);
            item.width += DETAIL_GAP + d[2];
            item.height = item.height.max(d[3]);
        }
    }
    rows.sort_by(|a, b| b.width.total_cmp(&a.width).then_with(|| a.name.cmp(&b.name)));

    let amount = |item: &Item| {
        if item.animation == 1.0 {
            1.0
        } else {
            smoothstep(item.animation)
        }
    };
    let box_w = rows
        .iter()
        .map(|i| i.width + padding_x * 2.0)
        .fold(0.0, f32::max);
    let box_h: f32 = rows
        .iter()
        .map(|i| amount(i) * (i.height + padding_y * 2.0))
        .sum();
    let [x, y] = ELEMENT.layout([sw, sh], [box_w, box_h]);
    let right = ELEMENT.right_aligned();
    let flush_side = if right { x + box_w >= sw - 0.5 } else { x <= 0.5 };
    let flush_top = y <= 0.5;
    let flush_bottom = y + box_h >= sh - 0.5;

    let mut cards = Vec::with_capacity(rows.len());
    let mut row_y = y;
    for i in 0..rows.len() {
        let item = &rows[i];
        let amount = amount(item);
        vg.global_alpha(amount * alpha);
        let (first, last) = (i == 0, i + 1 == rows.len());
        let next_step = if last {
            rounding
        } else {
            rounding.min(item.width - rows[i + 1].width)
        };
        let outer_top = if first && !flush_top { rounding } else { 0.0 };
        let outer_bottom = if last && flush_bottom { 0.0 } else { next_step };
        let inner_top = if first && !flush_top && !flush_side { rounding } else { 0.0 };
        let inner_bottom = if last && !flush_bottom && !flush_side { rounding } else { 0.0 };

        let card_h = item.height + padding_y * 2.0;
        let card_w = item.width + padding_x * 2.0;
        let slide = (1.0 - amount) * if flush_side { item.width } else { 12.0 };
        let card_x = if right { x + box_w - card_w + slide } else { x - slide };
        vg.begin_path();
        let radii = if right {
            [outer_top, inner_top, inner_bottom, outer_bottom]
        } else {
            [inner_top, outer_top, outer_bottom, inner_bottom]
        };
        vg.rounded_rect_varying(card_x, row_y, card_w, card_h, radii);
        vg.fill_color(fill);
        vg.fill();
        cards.push(backdrop::Card {
            x: card_x,
            y: row_y,
            w: card_w,
            h: card_h,
            radii,
            alpha: amount * alpha,
        });

        let (text_x, text_y) = (card_x + padding_x, row_y + padding_y);
        let color = gradient.map_or(name_color, |second| row_color(name_color, second, clock, i));
        vg.fill_color(Color::rgb(color));
        let name_end = vg.text(text_x, text_y, &item.name);
        if !item.detail.is_empty() {
            vg.fill_color(Color::rgb(details_color));
            vg.text(name_end + DETAIL_GAP, text_y, &item.detail);
        }
        row_y += amount * card_h;
    }
    vg.global_alpha(1.0);
    Ok(cards)
}

fn timewarp_extras() -> super::timewarp::Extras {
    use super::timewarp::{Extras, Item, Widget};
    let Some(g) = ADDITIONS.get() else {
        return Extras { before: None, items: Vec::new() };
    };
    let s = &g.settings;
    let mut items = vec![Item::Setting(s[GRADIENT].descriptor(), Widget::Bool)];
    if s[GRADIENT].flag() {
        items.push(Item::Setting(s[GRADIENT_COLOR].descriptor(), Widget::Color));
    }
    items.push(Item::Setting(s[BLUR].descriptor(), Widget::Bool));
    items.push(Item::Setting(s[BLOOM].descriptor(), Widget::Bool));
    items.push(Item::Setting(s[OPACITY].descriptor(), Widget::Float));
    Extras { before: None, items }
}
