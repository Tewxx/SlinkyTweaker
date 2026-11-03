use crate::gui::nanovg::{self, Color, Frame};
use crate::Result;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;

static AREA: Mutex<Option<([f32; 4], [f32; 2])>> = Mutex::new(None);
static BLUR: Mutex<Option<(i32, [f32; 2])>> = Mutex::new(None);

const BLUR_PASSES: i32 = 3;
const GLOW_PASSES: i32 = 3;
const BLUR_MARGIN: f32 = 16.0;
const GLOW_MARGIN: f32 = 32.0;

pub(super) struct Card {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub radii: [f32; 4],
    pub alpha: f32,
}

#[derive(Default)]
pub(super) struct Glow {
    key: u64,
    image: Option<i32>,
    rect: [f32; 4],
}

fn path(vg: &mut Frame<'_>, c: &Card) {
    vg.begin_path();
    vg.rounded_rect_varying(c.x, c.y, c.w, c.h, c.radii);
}

fn bounds(cards: &[Card]) -> [f32; 4] {
    let x0 = cards.iter().map(|c| c.x).fold(f32::MAX, f32::min);
    let y0 = cards.iter().map(|c| c.y).fold(f32::MAX, f32::min);
    let x1 = cards.iter().map(|c| c.x + c.w).fold(f32::MIN, f32::max);
    let y1 = cards.iter().map(|c| c.y + c.h).fold(f32::MIN, f32::max);
    [x0, y0, x1 - x0, y1 - y0]
}

fn grow([x, y, w, h]: [f32; 4], m: f32) -> [f32; 4] {
    [x - m, y - m, w + m * 2.0, h + m * 2.0]
}

pub(super) fn draw(cards: &[Card], [sw, sh]: [f32; 2], fill: Color, blur: bool, bloom: bool, glow: &mut Glow) -> Result<()> {
    if !bloom {
        glow.image = None;
    }
    let captured = BLUR.lock().unwrap_or_else(|e| e.into_inner()).take();
    let area = (!cards.is_empty()).then(|| bounds(cards));
    *AREA.lock().unwrap_or_else(|e| e.into_inner()) = area.filter(|_| blur).map(|a| (a, [sw, sh]));
    let Some(area) = area else {
        return Ok(());
    };
    if !(blur || bloom) {
        return Ok(());
    }
    let blur_image = captured.filter(|&(_, size)| blur && size == [sw, sh]).map(|(image, _)| image);
    if bloom {
        glow.update(cards, area, fill)?;
    }
    nanovg::draw(sw, sh, |vg| {
        if let (true, Some(image)) = (bloom, glow.image) {
            let [x, y, w, h] = glow.rect;
            vg.begin_path();
            vg.rect(x, y, w, h);
            vg.fill_image(glow.rect, image);
        }
        if let Some(image) = blur_image {
            for card in cards {
                vg.global_alpha(card.alpha);
                path(vg, card);
                vg.fill_blur(image);
            }
            vg.global_alpha(1.0);
        }
        Ok(())
    })
}

pub(super) fn capture([width, height]: [f32; 2]) -> Result<()> {
    let area = *AREA.lock().unwrap_or_else(|e| e.into_inner());
    let image = match area {
        Some(([x, y, w, h], [sw, sh])) if sw > 0.0 && sh > 0.0 => {
            let (kx, ky) = (width / sw, height / sh);
            let rect = grow([x * kx, y * ky, w * kx, h * ky], BLUR_MARGIN);
            nanovg::blur(width, height, rect, BLUR_PASSES)?.map(|image| (image, [sw, sh]))
        }
        _ => None,
    };
    *BLUR.lock().unwrap_or_else(|e| e.into_inner()) = image;
    Ok(())
}

impl Glow {
    fn update(&mut self, cards: &[Card], area: [f32; 4], fill: Color) -> Result<()> {
        let [x, y, w, h] = grow(area, GLOW_MARGIN);
        let rect = [x.floor(), y.floor(), w.ceil() + 1.0, h.ceil() + 1.0];
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let floats = cards
            .iter()
            .flat_map(|c| [c.x, c.y, c.w, c.h, c.alpha].into_iter().chain(c.radii))
            .chain(rect)
            .chain([fill.r, fill.g, fill.b, fill.a]);
        floats.for_each(|f| f.to_bits().hash(&mut hasher));
        let key = hasher.finish();
        if self.image.is_some() && key == self.key {
            return Ok(());
        }
        self.image = nanovg::glow(rect[2], rect[3], GLOW_PASSES, |vg| {
            vg.translate(-rect[0], -rect[1]);
            vg.fill_color(fill);
            for card in cards {
                vg.global_alpha(card.alpha);
                path(vg, card);
                vg.fill();
            }
        })?;
        self.key = key;
        self.rect = rect;
        Ok(())
    }
}
