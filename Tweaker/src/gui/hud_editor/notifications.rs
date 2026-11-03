use super::element::Element;
use super::{input, native, values, RUNTIME};
use crate::automap::{self, notifications::RING_REQUIRED};
use crate::gui::nanovg::{Color, Font, Frame, ALIGN_CENTER, ALIGN_LEFT, ALIGN_MIDDLE};
use crate::slinky::layout::{NOTIFY_BUF_LEN, NOTIFY_RING, SLOT_STRIDE, SLOT_TEXT_OFFSET, SLOT_TIMER_OFFSET};
use crate::slinky::mem::{read_cstr, read_f32, read_u32, read_u64};
use std::ffi::c_void;

const ORIGIN: f32 = 12.0;
pub(super) static ELEMENT: Element = Element::corner("notifications", "Notifications", [0.0, 0.0], ORIGIN);

const LIFETIME: f32 = 5.0;
const TRANSITION: f32 = 0.2;
const FADE_OUT_START: f32 = 4.8;
const CARD_H: f32 = 42.0;
const ROW_STEP: f32 = 50.0;
const BADGE_PADDING: f32 = 18.0;
const GREY_14: f32 = 0.078_431_375;
const GREY_DD: f32 = 0.866_666_7;
const FLAG_ALWAYS: u32 = 2;
const OFF_SCREEN: f32 = -100_000.0;

type Listener = unsafe extern "system" fn(*mut c_void);

struct Toast {
    kind: u32,
    text: String,
    timer: f32,
}

fn smoothstep(a: f32) -> f32 {
    a * a * (3.0 - 2.0 * a)
}
fn toast_alpha(timer: f32) -> f32 {
    let linear = if timer < TRANSITION {
        timer / TRANSITION
    } else if timer > FADE_OUT_START {
        (LIFETIME - timer) / TRANSITION
    } else {
        1.0
    };
    smoothstep(linear.clamp(0.0, 1.0))
}

fn shown() -> bool {
    values::enabled("Notifications") || values::flags("Notifications") & FLAG_ALWAYS != 0
}

fn toast_color(kind: u32) -> [f32; 3] {
    match kind {
        0 => [0.549_019_63, 0.925_490_2, 0.168_627_46],
        2 => [0.866_666_7, 0.278_431_4, 0.278_431_4],
        _ => crate::slinky::accent_color(),
    }
}

fn toasts() -> Vec<Toast> {
    let (Ok(map), Ok(())) = (automap::get(), automap::require(RING_REQUIRED)) else {
        return Vec::new();
    };
    let at = |name| map.base + automap::rva(name);
    let (Some(head), Some(count)) = (read_u64(at("queue_counter_a")), read_u64(at("queue_counter_b"))) else {
        return Vec::new();
    };
    (0..count.min(NOTIFY_RING))
        .filter_map(|i| {
            let slot = at("slot_array") + ((head + i) % NOTIFY_RING) as usize * SLOT_STRIDE;
            Some(Toast {
                kind: read_u32(slot)?,
                text: read_cstr(slot + SLOT_TEXT_OFFSET, NOTIFY_BUF_LEN)?,
                timer: read_f32(slot + SLOT_TIMER_OFFSET).filter(|t| t.is_finite())?,
            })
        })
        .filter(|t| t.timer <= LIFETIME)
        .collect()
}

fn text_width(vg: &mut Frame<'_>, text: &str) -> f32 {
    vg.font(Font::Medium);
    vg.font_size(14.0);
    vg.text_align(ALIGN_LEFT | ALIGN_MIDDLE);
    vg.text_advance(text)
}

fn badge_width(vg: &mut Frame<'_>, name: &str) -> f32 {
    (text_width(vg, name) + BADGE_PADDING).max(70.0)
}

fn card_width(vg: &mut Frame<'_>, name: &str, text: &str) -> f32 {
    badge_width(vg, name) + text_width(vg, text) + 32.0
}

fn stack_size(vg: &mut Frame<'_>, toasts: &[Toast], name: &str) -> [f32; 2] {
    let (mut w, mut y, mut bottom) = (0.0f32, 0.0f32, 0.0f32);
    for toast in toasts {
        let alpha = toast_alpha(toast.timer);
        w = w.max(card_width(vg, name, &toast.text));
        bottom = bottom.max(y + CARD_H);
        y += alpha * ROW_STEP;
    }
    [w, bottom]
}

#[allow(clippy::too_many_arguments)]
fn draw_toast(vg: &mut Frame<'_>, x: f32, y: f32, width: f32, name: &str, text: &str, kind: u32, alpha: f32) {
    let color = toast_color(kind);
    let badge = badge_width(vg, name);
    vg.global_alpha(alpha);
    vg.begin_path();
    vg.rounded_rect(x, y, width, CARD_H, 20.0);
    vg.fill_color(Color::rgba(GREY_14, GREY_14, GREY_14, 1.0));
    vg.fill();
    vg.begin_path();
    vg.rounded_rect(x + 8.0, y + 8.0, badge, 26.0, 10.0);
    vg.fill_color(Color::rgba(color[0], color[1], color[2], 0.15));
    vg.fill();
    vg.font(Font::Medium);
    vg.font_size(14.0);
    vg.text_align(ALIGN_CENTER | ALIGN_MIDDLE);
    vg.fill_color(Color::rgb(color));
    vg.text(x + 8.0 + badge * 0.5, y + 8.0 + 1.0 + 13.0, name);
    vg.text_align(ALIGN_LEFT | ALIGN_MIDDLE);
    vg.fill_color(Color::rgba(GREY_DD, GREY_DD, GREY_DD, 1.0));
    vg.text(x + 16.0 + badge, y + 1.0 + CARD_H * 0.5, text);
    vg.global_alpha(1.0);
}

fn draw_stack(vg: &mut Frame<'_>, toasts: &[Toast], name: &str, [x, y]: [f32; 2], size: [f32; 2], right: bool) {
    let mut row_y = y;
    for toast in toasts {
        let alpha = toast_alpha(toast.timer);
        let width = card_width(vg, name, &toast.text);
        let card_x = if right {
            x + size[0] - width + (1.0 - alpha) * 100.0
        } else {
            x - (1.0 - alpha) * 100.0
        };
        draw_toast(vg, card_x, row_y, width, name, &toast.text, toast.kind, alpha);
        row_y += alpha * ROW_STEP;
    }
}

pub(super) unsafe extern "system" fn render(event: *mut c_void) {
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    let Some(original) = runtime.notifications_render else {
        return;
    };
    let original = unsafe { std::mem::transmute::<usize, Listener>(original) };
    let (Some(ctx), Some(screen)) = (native::ui_context(event), input::client_size()) else {
        return unsafe { original(event) };
    };
    unsafe { crate::gui::nanovg::translated(ctx, OFF_SCREEN, OFF_SCREEN, || original(event)) };
    let toasts = toasts();
    let name = super::custom_name::name();
    let menu_visible = values::enabled("GUI");
    let result = unsafe {
        crate::gui::nanovg::draw_native(ctx, |vg| {
            if !shown() {
                return Ok(());
            }
            if toasts.is_empty() {
                if super::editing() {
                    const PREVIEW: &str = "Enabled Velocity";
                    let w = card_width(vg, &name, PREVIEW);
                    let at = ELEMENT.layout(screen, [w, CARD_H]);
                    draw_toast(vg, at[0], at[1], w, &name, PREVIEW, 1, 0.6);
                }
                return Ok(());
            }
            if !menu_visible && values::flag("Notifications", "hide_ingame", false) {
                return Ok(());
            }
            let size = stack_size(vg, &toasts, &name);
            let at = ELEMENT.layout(screen, size);
            draw_stack(vg, &toasts, &name, at, size, ELEMENT.right_aligned());
            Ok(())
        })
    };
    let _ = result;
}
