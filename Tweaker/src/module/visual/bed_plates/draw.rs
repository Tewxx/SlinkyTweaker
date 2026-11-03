use super::scan::Defense;
use crate::minecraft::bed_plates::Icon;
use crate::gui::nanovg::{Color, Font, Frame, ALIGN_LEFT, ALIGN_TOP};

#[derive(Clone, Copy)]
pub struct View {
    pub modelview: [f32; 16],
    pub projection: [f32; 16],
    pub size: [f32; 2],
    pub origin: [f64; 3],
}
impl View {
    pub fn project(&self, point: [f64; 3]) -> Option<[f32; 2]> {
        let [px, py, pz] = [0, 1, 2].map(|i| (point[i] - self.origin[i]) as f32);
        let p = &self.modelview;
        let c0 = p[0] * px + p[4] * py + p[8] * pz + p[12];
        let c1 = p[1] * px + p[5] * py + p[9] * pz + p[13];
        let c2 = p[2] * px + p[6] * py + p[10] * pz + p[14];
        let c3 = p[3] * px + p[7] * py + p[11] * pz + p[15];
        let m = &self.projection;
        let w = m[3] * c0 + m[7] * c1 + m[11] * c2 + m[15] * c3;
        if w == 0.0 {
            return None;
        }
        let qw = (1.0 / w) * 0.5;
        let depth = (m[2] * c0 + m[6] * c1 + m[10] * c2 + m[14] * c3) * qw + 0.5;
        if !(0.0 < depth && depth < 1.0) {
            return None;
        }
        let ndc_y = (m[1] * c0 + m[5] * c1 + m[9] * c2 + m[13] * c3) * qw + 0.5;
        let ndc_x = (m[0] * c0 + m[4] * c1 + m[8] * c2 + m[12] * c3) * qw + 0.5;
        let [w, h] = self.size;
        Some([ndc_x * w, h - ndc_y * h])
    }
}

pub struct Plate<'a> {
    pub x: f32,
    pub y: f32,
    pub defense: &'a Defense,
}

pub fn plates<'a>(view: &View, defenses: &'a [Defense], [width, height]: [f32; 2]) -> Vec<Plate<'a>> {
    let [sx, sy] = [width / view.size[0], height / view.size[1]];
    defenses
        .iter()
        .filter_map(|defense| {
            let [x, y, z] = defense.bed.centre();
            let [x, y] = view.project([x, y + 1.1, z])?;
            let [x, y] = [x * sx, y * sy];
            let on_screen = x >= -200.0 && y >= -200.0 && x <= width + 200.0 && y <= height + 200.0;
            on_screen.then_some(Plate { x, y, defense })
        })
        .collect()
}

struct Layout {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    icon: f32,
    gap: f32,
    pad: f32,
    text_size: f32,
    text_w: f32,
}

fn layout(vg: &mut Frame<'_>, plate: &Plate<'_>, s: f32) -> Layout {
    let (icon, gap, pad, text_size) = (20.0 * s, 4.0 * s, 8.0 * s, 15.0 * s);
    let d = plate.defense;
    let text_w = d.status.map_or(0.0, |t| {
        vg.font(Font::Bold);
        vg.font_size(text_size);
        vg.text_align(ALIGN_LEFT | ALIGN_TOP);
        let b = vg.text_bounds(0.0, 0.0, t);
        b[2] - b[0]
    });
    let icons_w = icons_width(d.icons.len(), icon, gap);
    let has_text = d.status.is_some();
    let text_h = if has_text { text_size * 1.2 } else { 0.0 };
    let icons_h = if d.icons.is_empty() { 0.0 } else { icon };
    let between = if has_text && !d.icons.is_empty() { 3.0 * s } else { 0.0 };
    let w = icons_w.max(text_w) + pad * 2.0 + if has_text { 2.0 * s } else { 0.0 };
    let h = icons_h + text_h + between + pad * 2.0 - if has_text { 2.0 * s } else { 0.0 };
    Layout {
        x: (plate.x - w * 0.5).round(),
        y: (plate.y - h * 0.5).round(),
        w,
        h,
        icon,
        gap,
        pad,
        text_size,
        text_w,
    }
}

fn icons_width(count: usize, icon: f32, gap: f32) -> f32 {
    if count == 0 {
        0.0
    } else {
        count as f32 * (icon + gap) - gap
    }
}

pub fn pills(vg: &mut Frame<'_>, plates: &[Plate<'_>], scale: f32, blur: Option<i32>) -> Vec<Icon> {
    let mut icons = Vec::new();
    for plate in plates {
        let l = layout(vg, plate, scale);
        let radius = 8.0 * scale;
        if let Some(image) = blur {
            vg.begin_path();
            vg.rounded_rect(l.x, l.y, l.w, l.h, radius);
            vg.fill_blur(image);
        }
        vg.begin_path();
        vg.rounded_rect(l.x, l.y, l.w, l.h, radius);
        vg.fill_color(Color::rgba(0.08, 0.07, 0.07, if blur.is_some() { 0.55 } else { 0.7 }));
        vg.fill();
        let d = plate.defense;
        let mut cursor_y = l.y + l.pad;
        if !d.icons.is_empty() {
            let mut x = l.x + (l.w - icons_width(d.icons.len(), l.icon, l.gap)) * 0.5;
            for &state in &d.icons {
                icons.push(Icon {
                    x: x.round(),
                    y: cursor_y.round(),
                    size: l.icon,
                    state,
                });
                x += l.icon + l.gap;
            }
            cursor_y += l.icon + 3.0 * scale;
        }
        if let Some(text) = d.status {
            vg.font(Font::Bold);
            vg.font_size(l.text_size);
            vg.text_align(ALIGN_LEFT | ALIGN_TOP);
            vg.fill_color(Color::rgba(0.96, 0.96, 0.96, 1.0));
            vg.text(
                (l.x + (l.w - l.text_w) * 0.5).round(),
                (cursor_y - scale).round(),
                text,
            );
        }
    }
    icons
}

const CUBE_TRIANGLES: [u8; 36] = [
    0, 1, 3, 0, 3, 2, 4, 6, 7, 4, 7, 5, 0, 4, 5, 0, 5, 1, 2, 3, 7, 2, 7, 6, 0, 2, 6, 0, 6, 4, 1, 5,
    7, 1, 7, 3,
];

pub unsafe fn highlight(defenses: &[Defense], origin: [f64; 3]) {
    if defenses.is_empty() {
        return;
    }
    let mut vertices = Vec::with_capacity(defenses.len() * 24);
    let mut indices = Vec::with_capacity(defenses.len() * 36);
    for (i, defense) in defenses.iter().enumerate() {
        let [head, foot] = defense.bed.blocks();
        let low = [
            head[0].min(foot[0]) as f64,
            head[1] as f64,
            head[2].min(foot[2]) as f64,
        ];
        let high = [
            head[0].max(foot[0]) as f64 + 1.0,
            head[1] as f64 + 0.5625,
            head[2].max(foot[2]) as f64 + 1.0,
        ];
        for corner in 0..8 {
            for (axis, bit) in [(0, 4), (1, 2), (2, 1)] {
                let v = if corner & bit != 0 { high[axis] } else { low[axis] };
                vertices.push((v - origin[axis]) as f32);
            }
        }
        let base = (i * 8) as u16;
        indices.extend(CUBE_TRIANGLES.iter().map(|&n| base + n as u16));
    }
    unsafe {
        gl::glPushAttrib(gl::ALL_ATTRIB_BITS);
        gl::glPushClientAttrib(gl::CLIENT_ALL_ATTRIB_BITS);
        for cap in [gl::DEPTH_TEST, gl::TEXTURE_2D, gl::LIGHTING, gl::CULL_FACE, gl::ALPHA_TEST] {
            gl::glDisable(cap);
        }
        gl::glEnable(gl::BLEND);
        gl::glBlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        gl::glColor4f(1.0, 0.27, 0.27, 0.38);
        gl::glEnableClientState(gl::VERTEX_ARRAY);
        gl::glVertexPointer(3, gl::FLOAT, 0, vertices.as_ptr().cast());
        gl::glDrawElements(gl::TRIANGLES, indices.len() as i32, gl::UNSIGNED_SHORT, indices.as_ptr().cast());
        gl::glPopClientAttrib();
        gl::glPopAttrib();
    }
}

pub unsafe fn capture(origin: [f64; 3]) -> Option<View> {
    let mut view = View {
        modelview: [0.0; 16],
        projection: [0.0; 16],
        size: [0.0; 2],
        origin,
    };
    let mut viewport = [0i32; 4];
    unsafe {
        gl::glGetIntegerv(gl::VIEWPORT, viewport.as_mut_ptr());
        gl::glGetFloatv(gl::MODELVIEW_MATRIX, view.modelview.as_mut_ptr());
        gl::glGetFloatv(gl::PROJECTION_MATRIX, view.projection.as_mut_ptr());
    }
    view.size = [viewport[2] as f32, viewport[3] as f32];
    (view.size.iter().all(|&v| v > 0.0) && view.modelview.iter().chain(&view.projection).all(|v| v.is_finite()))
        .then_some(view)
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
    pub const SRC_ALPHA: u32 = 0x0302;
    pub const ONE_MINUS_SRC_ALPHA: u32 = 0x0303;
    pub const VERTEX_ARRAY: u32 = 0x8074;
    pub const FLOAT: u32 = 0x1406;
    pub const TRIANGLES: u32 = 0x0004;
    pub const UNSIGNED_SHORT: u32 = 0x1403;
    pub const VIEWPORT: u32 = 0x0BA2;
    pub const MODELVIEW_MATRIX: u32 = 0x0BA6;
    pub const PROJECTION_MATRIX: u32 = 0x0BA7;

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
        pub fn glEnableClientState(array: u32);
        pub fn glVertexPointer(size: i32, kind: u32, stride: i32, data: *const c_void);
        pub fn glDrawElements(mode: u32, count: i32, kind: u32, indices: *const c_void);
        pub fn glGetIntegerv(name: u32, out: *mut i32);
        pub fn glGetFloatv(name: u32, out: *mut f32);
    }
}
