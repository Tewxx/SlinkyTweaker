use super::element::Rect;
use super::input::Mouse;
use super::overlay::{bottom_button_gap, draw_text, ease_open, fill_rrect, grey, smooth_toward, smoothstep, GREY_14, GREY_40, GREY_DD};
use super::visibility;
use crate::gui::nanovg::{Color, Font, Frame, BUTT, CCW, MITER, ROUND};
use crate::slinky::mem::{read_f32, read_memory, read_u32, read_u64};
use iced_x86::{Decoder, DecoderOptions, Mnemonic, Register};

const GUI_PAGE: usize = 0x350;
const GUI_PAGE_ANIM: usize = 0x358;
const GUI_PAGE_TYPES: usize = 0x2e0;
const GUI_OPEN: usize = 0x4b8;
const GUI_SCROLL: usize = 0x4bc;
const GUI_WIDE: usize = 0x4cd;
const CARD_HEIGHT: usize = 0x38;
const TAU: f32 = 0.06;

pub(super) fn gui_object(gui_render: usize) -> Option<usize> {
    let code = read_memory(gui_render, 0x80)?;
    let mut decoder = Decoder::with_ip(64, &code, gui_render as u64, DecoderOptions::NONE);
    for inst in decoder.iter().take(32) {
        if inst.mnemonic() == Mnemonic::Lea && inst.op0_register() == Register::RCX && inst.is_ip_rel_memory_operand() {
            let gui = inst.ip_rel_memory_address() as usize;
            let page = read_u64(gui + GUI_PAGE)?;
            let open = read_f32(gui + GUI_OPEN)?;
            return (page < 7 && (0.0..=1.0).contains(&open)).then_some(gui);
        }
        if inst.mnemonic() == Mnemonic::Ret {
            break;
        }
    }
    None
}

struct Columns {
    x: f32,
    width: f32,
    top: f32,
    n: usize,
    slots: [Rect; 4],
    count: usize,
}
impl Columns {
    const SPACING: f32 = 8.0;

    fn next(&mut self, h: f32) -> Rect {
        let (i, n) = (self.count, self.n);
        self.count += 1;
        let prev = self.slots[i.wrapping_sub(1) & 3];
        let above = self.slots[i.wrapping_sub(n) & 3];
        let mut cur = Rect { x: self.x, y: self.top, w: 0.0, h };
        if i % n != 0 {
            cur.x = prev.x + prev.w + Self::SPACING;
            cur.w = prev.w;
            cur.y = if i < n { self.top } else { above.y + above.h + Self::SPACING };
        } else if i < n {
            cur.w = (self.width - (n - 1) as f32 * Self::SPACING) / n as f32;
        } else {
            cur.y = above.y + above.h + Self::SPACING;
            cur.w = above.w;
        }
        self.slots[i & 3] = cur;
        Rect { x: cur.x.round(), y: cur.y.round(), w: cur.w.round(), h: cur.h.round() }
    }
}

fn card_rects(screen_w: f32, wide: bool, heights: &[f32]) -> Vec<Rect> {
    let width = (screen_w - 16.0).min(if wide { 1500.0 } else { 900.0 });
    let (x, inner_w) = ((screen_w - width) * 0.5 + 12.0, width - 24.0);
    let top = 12.0 + 42.0 + Columns::SPACING;
    let n = (((inner_w + Columns::SPACING) / (430.0 + Columns::SPACING)) as usize).clamp(1, 4);
    let mut columns = Columns { x, width: inner_w, top, n, slots: [Rect::default(); 4], count: 0 };
    heights.iter().map(|&h| columns.next(h)).collect()
}

struct Card {
    name: String,
    rect: Rect,
}

fn page(gui: usize, manager: usize, screen: [f32; 2]) -> Option<(Vec<Card>, f32)> {
    let index = read_u64(gui + GUI_PAGE)? as usize;
    if index >= 7 {
        return None;
    }
    let kind = read_u32(gui + GUI_PAGE_TYPES + index * 0x10)? as usize;
    if kind >= visibility::CATEGORIES.len() {
        return None;
    }
    let modules = visibility::page(manager, kind);
    let heights: Vec<f32> = modules.iter().map(|(d, _)| read_f32(d + CARD_HEIGHT).unwrap_or(0.0)).collect();
    let wide = read_memory(gui + GUI_WIDE, 1)?[0] == 1;
    let open = ease_open(read_f32(gui + GUI_OPEN)?);
    let scroll = read_f32(gui + GUI_SCROLL)?;
    let diff = index as f32 - read_f32(gui + GUI_PAGE_ANIM)?;
    let alpha = if diff.abs() > 0.5 { 0.0 } else { 1.0 - 2.0 * diff.abs() };
    let dx = diff * screen[0] * -0.1;
    let dy = -50.0 + 50.0 * open - scroll;
    let cards = card_rects(screen[0], wide, &heights)
        .into_iter()
        .zip(modules)
        .map(|(r, (_, name))| Card { name, rect: Rect { x: r.x + dx, y: r.y + dy, ..r } })
        .filter(|c| c.rect.h > 0.0 && c.rect.y < screen[1] && c.rect.y + c.rect.h > 0.0)
        .collect();
    Some((cards, alpha * open))
}

pub(super) fn pencil_icon(vg: &mut Frame<'_>, x: f32, y: f32, size: f32, color: Color) {
    let s = size / 24.0;
    let p = |px: f32, py: f32| (x + px * s, y + py * s);
    stroke_icon(vg, size, 2.25, color, |vg| {
        let (cx, cy) = p(19.181, 4.8185);
        let start = p(21.174, 6.812);
        vg.move_to(start.0, start.1);
        vg.arc(cx, cy, 2.819 * s, std::f32::consts::FRAC_PI_4, -3.0 * std::f32::consts::FRAC_PI_4, CCW);
        for (px, py) in [(3.842, 16.174), (3.342, 17.004), (2.021, 21.356), (2.644, 21.978), (6.997, 20.658), (7.827, 20.161)] {
            let (px, py) = p(px, py);
            vg.line_to(px, py);
        }
        vg.close_path();
        let (ax, ay) = p(15.0, 5.0);
        let (bx, by) = p(19.0, 9.0);
        vg.move_to(ax, ay);
        vg.line_to(bx, by);
    });
}

fn tick_icon(vg: &mut Frame<'_>, x: f32, y: f32, size: f32, color: Color) {
    let s = size / 24.0;
    stroke_icon(vg, size, 2.75, color, |vg| {
        vg.move_to(x + 20.0 * s, y + 6.0 * s);
        vg.line_to(x + 9.0 * s, y + 17.0 * s);
        vg.line_to(x + 4.0 * s, y + 12.0 * s);
    });
}

fn cross_icon(vg: &mut Frame<'_>, x: f32, y: f32, size: f32, color: Color) {
    let s = size / 24.0;
    stroke_icon(vg, size, 2.75, color, |vg| {
        vg.move_to(x + 18.0 * s, y + 6.0 * s);
        vg.line_to(x + 6.0 * s, y + 18.0 * s);
        vg.move_to(x + 6.0 * s, y + 6.0 * s);
        vg.line_to(x + 18.0 * s, y + 18.0 * s);
    });
}

fn stroke_icon(vg: &mut Frame<'_>, size: f32, width: f32, color: Color, path: impl FnOnce(&mut Frame<'_>)) {
    vg.begin_path();
    path(vg);
    vg.stroke_color(color);
    vg.stroke_width((width * size / 24.0).max(1.0));
    vg.line_cap(ROUND);
    vg.line_join(ROUND);
    vg.stroke();
    vg.line_cap(BUTT);
    vg.line_join(MITER);
}

#[derive(Default)]
pub(super) struct ModuleEdit {
    pub on: bool,
    anim: f32,
}

impl ModuleEdit {
    pub fn reset(&mut self) {
        self.on = false;
        self.anim = 0.0;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, vg: &mut Frame<'_>, mouse: Mouse, gui: usize, manager: usize, screen: [f32; 2], scale: f32, dt: f32, accent: Color) -> bool {
        let target = if self.on { 1.0 } else { 0.0 };
        self.anim = smooth_toward(self.anim, target, dt / TAU);
        if (self.anim - target).abs() < 0.001 {
            self.anim = target;
        }
        if self.anim <= 0.0 {
            return false;
        }
        let (sw, sh) = (screen[0] / scale, screen[1] / scale);
        let cursor = (self.on && mouse.inside).then(|| [mouse.x / scale, mouse.y / scale]);
        let at = |r: Rect| cursor.is_some_and(|[x, y]| r.contains(x, y));
        let fade = smoothstep(self.anim);
        let mut hot = false;
        vg.save();
        vg.scale(scale, scale);
        if let Some((cards, alpha)) = page(gui, manager, [sw, sh]) {
            vg.global_alpha(alpha * fade);
            for card in &cards {
                let r = card.rect;
                let shown = !visibility::is_hidden(&card.name);
                let hovered = alpha > 0.5 && at(r);
                hot |= hovered;
                if !shown {
                    fill_rrect(vg, r, 20.0, grey(GREY_14, 0.7));
                }
                if hovered {
                    fill_rrect(vg, r, 20.0, grey(1.0, 0.04));
                }
                let toggle = Rect { x: r.x + r.w - 12.0 - 40.0, y: r.y + 12.0, w: 40.0, h: 20.0 };
                let (fill, icon) = if shown { (accent, grey(1.0, 1.0)) } else { (grey(GREY_40, 1.0), grey(GREY_DD, 1.0)) };
                fill_rrect(vg, toggle, 10.0, fill);
                let (ix, iy) = (toggle.x + 13.0, toggle.y + 3.0);
                if shown {
                    tick_icon(vg, ix, iy, 14.0, icon);
                } else {
                    cross_icon(vg, ix, iy, 14.0, icon);
                }
                if hovered && mouse.left.pressed {
                    visibility::set_hidden(&card.name, shown);
                }
            }
        }

        vg.global_alpha(fade);
        let (done_w, done_h) = (110.0, 36.0);
        let done = Rect { x: (sw - done_w) * 0.5, y: sh - done_h - bottom_button_gap(scale), w: done_w, h: done_h };
        let done_hovered = at(done);
        hot |= done_hovered;
        let fill = if done_hovered { Color { r: accent.r + (1.0 - accent.r) * 0.15, g: accent.g + (1.0 - accent.g) * 0.15, b: accent.b + (1.0 - accent.b) * 0.15, a: 1.0 } } else { accent };
        fill_rrect(vg, done, done_h * 0.5, fill);
        draw_text(vg, done, true, Font::Bold, 15.0, grey(1.0, 1.0), "Done");
        if done_hovered && mouse.left.pressed {
            self.on = false;
        }
        vg.restore();
        hot
    }
}
