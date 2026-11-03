use crate::error::{Error, Result};
use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void};

pub const ALIGN_LEFT: i32 = 1;
pub const ALIGN_CENTER: i32 = 2;
pub const ALIGN_TOP: i32 = 8;
pub const ALIGN_MIDDLE: i32 = 16;

pub const BUTT: i32 = 0;
pub const ROUND: i32 = 1;
pub const MITER: i32 = 4;
pub const CCW: i32 = 1;

const ANTIALIAS: c_int = 1;
static INTER_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Inter-Medium.ttf");
static INTER_BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-Bold.ttf");

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
impl Color {
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
    pub const fn rgb([r, g, b]: [f32; 3]) -> Self {
        Self::rgba(r, g, b, 1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    Medium,
    Bold,
}

#[repr(C)]
#[derive(Default)]
struct Saved {
    program: c_int,
    array_buffer: c_int,
    active_texture: c_int,
}

type Ctx = *mut c_void;
extern "C" {
    fn nvgrs_load_gl() -> c_int;
    fn nvgrs_create(flags: c_int) -> Ctx;
    fn nvgrs_save(saved: *mut Saved);
    fn nvgrs_restore(saved: *const Saved);
    fn nvgrs_begin(ctx: Ctx, saved: *mut Saved, width: f32, height: f32);
    fn nvgrs_end(ctx: Ctx, saved: *const Saved);
    fn nvgCreateFontMem(
        ctx: Ctx,
        name: *const c_char,
        data: *mut u8,
        len: c_int,
        free_data: c_int,
    ) -> c_int;
    fn nvgSave(ctx: Ctx);
    fn nvgRestore(ctx: Ctx);
    fn nvgTranslate(ctx: Ctx, x: f32, y: f32);
    fn nvgScale(ctx: Ctx, x: f32, y: f32);
    fn nvgGlobalAlpha(ctx: Ctx, alpha: f32);
    fn nvgFillColor(ctx: Ctx, color: Color);
    fn nvgStrokeColor(ctx: Ctx, color: Color);
    fn nvgStrokeWidth(ctx: Ctx, width: f32);
    fn nvgBeginPath(ctx: Ctx);
    fn nvgRect(ctx: Ctx, x: f32, y: f32, w: f32, h: f32);
    fn nvgRoundedRect(ctx: Ctx, x: f32, y: f32, w: f32, h: f32, r: f32);
    #[allow(clippy::too_many_arguments)]
    fn nvgRoundedRectVarying(
        ctx: Ctx,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        top_left: f32,
        top_right: f32,
        bottom_right: f32,
        bottom_left: f32,
    );
    fn nvgMoveTo(ctx: Ctx, x: f32, y: f32);
    fn nvgLineTo(ctx: Ctx, x: f32, y: f32);
    fn nvgArc(ctx: Ctx, cx: f32, cy: f32, r: f32, a0: f32, a1: f32, dir: c_int);
    fn nvgClosePath(ctx: Ctx);
    fn nvgFill(ctx: Ctx);
    fn nvgStroke(ctx: Ctx);
    fn nvgLineCap(ctx: Ctx, cap: c_int);
    fn nvgLineJoin(ctx: Ctx, join: c_int);
    fn nvgCurrentTransform(ctx: Ctx, transform: *mut f32);
    fn nvgResetTransform(ctx: Ctx);
    fn nvgResetScissor(ctx: Ctx);
    fn nvgFindFont(ctx: Ctx, name: *const c_char) -> c_int;
    fn nvgFontFaceId(ctx: Ctx, font: c_int);
    fn nvgFontSize(ctx: Ctx, size: f32);
    fn nvgTextAlign(ctx: Ctx, align: c_int);
    fn nvgText(ctx: Ctx, x: f32, y: f32, start: *const c_char, end: *const c_char) -> f32;
    #[allow(clippy::too_many_arguments)]
    fn nvgrs_blur(ctx: Ctx, width: c_int, height: c_int, x: c_int, y: c_int, w: c_int, h: c_int, passes: c_int) -> c_int;
    fn nvgrs_layer_begin(ctx: Ctx, saved: *mut Saved, width: c_int, height: c_int) -> c_int;
    fn nvgrs_layer_end(ctx: Ctx, saved: *const Saved);
    fn nvgrs_layer_blur(passes: c_int) -> c_int;
    fn nvgrs_fill_image(ctx: Ctx, x: f32, y: f32, w: f32, h: f32, image: c_int, alpha: f32);
    fn nvgTextBounds(
        ctx: Ctx,
        x: f32,
        y: f32,
        start: *const c_char,
        end: *const c_char,
        bounds: *mut f32,
    ) -> f32;
}

struct Canvas {
    ctx: Ctx,
    fonts: [c_int; 2],
}
impl Canvas {
    unsafe fn create() -> std::result::Result<Self, String> {
        unsafe {
            let missing = nvgrs_load_gl();
            if missing != 0 {
                return Err(format!("NanoVG GL entry point #{missing} unavailable"));
            }
            let mut saved = Saved::default();
            nvgrs_save(&mut saved);
            let ctx = nvgrs_create(ANTIALIAS);
            let fonts = if ctx.is_null() {
                [-1; 2]
            } else {
                let load = |name: &[u8], data: &'static [u8]| {
                    nvgCreateFontMem(
                        ctx,
                        name.as_ptr().cast(),
                        data.as_ptr().cast_mut(),
                        data.len() as c_int,
                        0,
                    )
                };
                [
                    load(b"inter\0", INTER_MEDIUM),
                    load(b"inter-bold\0", INTER_BOLD),
                ]
            };
            nvgrs_restore(&saved);
            if ctx.is_null() {
                return Err("NanoVG GL2 context creation failed".into());
            }
            if fonts.iter().any(|&f| f < 0) {
                return Err("NanoVG could not load its fonts".into());
            }
            Ok(Self { ctx, fonts })
        }
    }
}

thread_local! {
    static CANVAS: RefCell<Option<std::result::Result<Canvas, String>>> = const { RefCell::new(None) };
}

pub struct Frame<'a> {
    ctx: Ctx,
    fonts: [c_int; 2],
    own: Option<[f32; 2]>,
    _frame: std::marker::PhantomData<&'a ()>,
}
impl Frame<'_> {
    pub fn save(&mut self) {
        unsafe { nvgSave(self.ctx) }
    }
    pub fn restore(&mut self) {
        unsafe { nvgRestore(self.ctx) }
    }
    pub fn translate(&mut self, x: f32, y: f32) {
        unsafe { nvgTranslate(self.ctx, x, y) }
    }
    pub fn scale(&mut self, x: f32, y: f32) {
        unsafe { nvgScale(self.ctx, x, y) }
    }
    pub fn current_transform(&mut self) -> [f32; 6] {
        let mut transform = [0.0; 6];
        unsafe { nvgCurrentTransform(self.ctx, transform.as_mut_ptr()) };
        transform
    }
    pub fn global_alpha(&mut self, alpha: f32) {
        unsafe { nvgGlobalAlpha(self.ctx, alpha) }
    }
    pub fn fill_color(&mut self, color: Color) {
        unsafe { nvgFillColor(self.ctx, color) }
    }
    pub fn stroke_color(&mut self, color: Color) {
        unsafe { nvgStrokeColor(self.ctx, color) }
    }
    pub fn stroke_width(&mut self, width: f32) {
        unsafe { nvgStrokeWidth(self.ctx, width) }
    }
    pub fn begin_path(&mut self) {
        unsafe { nvgBeginPath(self.ctx) }
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        unsafe { nvgRect(self.ctx, x, y, w, h) }
    }
    pub fn rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32) {
        unsafe { nvgRoundedRect(self.ctx, x, y, w, h, radius) }
    }
    pub fn rounded_rect_varying(&mut self, x: f32, y: f32, w: f32, h: f32, [tl, tr, br, bl]: [f32; 4]) {
        unsafe { nvgRoundedRectVarying(self.ctx, x, y, w, h, tl, tr, br, bl) }
    }
    pub fn move_to(&mut self, x: f32, y: f32) {
        unsafe { nvgMoveTo(self.ctx, x, y) }
    }
    pub fn line_to(&mut self, x: f32, y: f32) {
        unsafe { nvgLineTo(self.ctx, x, y) }
    }
    pub fn arc(&mut self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32, dir: i32) {
        unsafe { nvgArc(self.ctx, cx, cy, r, a0, a1, dir) }
    }
    pub fn close_path(&mut self) {
        unsafe { nvgClosePath(self.ctx) }
    }
    pub fn fill(&mut self) {
        unsafe { nvgFill(self.ctx) }
    }
    pub fn stroke(&mut self) {
        unsafe { nvgStroke(self.ctx) }
    }
    pub fn blur(&mut self, passes: i32) -> Option<i32> {
        let [w, h] = self.own?;
        self.blur_region([0.0, 0.0, w, h], passes)
    }
    pub fn blur_region(&mut self, [x, y, w, h]: [f32; 4], passes: i32) -> Option<i32> {
        let [fw, fh] = self.own?;
        let (x0, y0) = (x.floor(), y.floor());
        let (x1, y1) = ((x + w).ceil(), (y + h).ceil());
        let image = unsafe {
            nvgrs_blur(
                self.ctx,
                fw as c_int,
                fh as c_int,
                x0 as c_int,
                y0 as c_int,
                (x1 - x0) as c_int,
                (y1 - y0) as c_int,
                passes,
            )
        };
        (image != 0).then_some(image)
    }
    pub fn fill_blur(&mut self, image: i32) {
        if let Some([w, h]) = self.own {
            unsafe { nvgrs_fill_image(self.ctx, 0.0, 0.0, w, h, image, 1.0) }
        }
    }
    pub fn fill_image(&mut self, [x, y, w, h]: [f32; 4], image: i32) {
        unsafe { nvgrs_fill_image(self.ctx, x, y, w, h, image, 1.0) }
    }
    pub fn line_cap(&mut self, cap: i32) {
        unsafe { nvgLineCap(self.ctx, cap) }
    }
    pub fn line_join(&mut self, join: i32) {
        unsafe { nvgLineJoin(self.ctx, join) }
    }
    pub fn font(&mut self, font: Font) {
        unsafe { nvgFontFaceId(self.ctx, self.fonts[font as usize]) }
    }
    pub fn font_size(&mut self, size: f32) {
        unsafe { nvgFontSize(self.ctx, size) }
    }
    pub fn text_align(&mut self, align: i32) {
        unsafe { nvgTextAlign(self.ctx, align) }
    }
    pub fn text(&mut self, x: f32, y: f32, text: &str) -> f32 {
        let range = text.as_bytes().as_ptr_range();
        unsafe { nvgText(self.ctx, x, y, range.start.cast(), range.end.cast()) }
    }
    pub fn text_advance(&mut self, text: &str) -> f32 {
        let range = text.as_bytes().as_ptr_range();
        unsafe {
            nvgTextBounds(
                self.ctx,
                0.0,
                0.0,
                range.start.cast(),
                range.end.cast(),
                std::ptr::null_mut(),
            )
        }
    }
    pub fn text_bounds(&mut self, x: f32, y: f32, text: &str) -> [f32; 4] {
        let range = text.as_bytes().as_ptr_range();
        let mut bounds = [0.0; 4];
        unsafe {
            nvgTextBounds(
                self.ctx,
                x,
                y,
                range.start.cast(),
                range.end.cast(),
                bounds.as_mut_ptr(),
            );
        }
        bounds
    }
}

pub fn draw(width: f32, height: f32, f: impl FnOnce(&mut Frame<'_>) -> Result<()>) -> Result<()> {
    if !(width.is_finite() && height.is_finite() && width >= 1.0 && height >= 1.0) {
        return Ok(());
    }
    CANVAS.with(|cell| {
        let mut entry = cell
            .try_borrow_mut()
            .map_err(|_| Error::Jvm("NanoVG frame already active".into()))?;
        let canvas = entry
            .get_or_insert_with(|| unsafe { Canvas::create() })
            .as_ref()
            .map_err(|e| Error::Jvm(e.clone()))?;
        struct End<'a>(&'a Canvas, Saved);
        impl Drop for End<'_> {
            fn drop(&mut self) {
                unsafe { nvgrs_end(self.0.ctx, &self.1) }
            }
        }
        let mut end = End(canvas, Saved::default());
        unsafe { nvgrs_begin(canvas.ctx, &mut end.1, width, height) };
        f(&mut Frame {
            ctx: canvas.ctx,
            fonts: canvas.fonts,
            own: Some([width, height]),
            _frame: std::marker::PhantomData,
        })
    })
}

pub fn blur(width: f32, height: f32, rect: [f32; 4], passes: i32) -> Result<Option<i32>> {
    if !(width.is_finite() && height.is_finite() && width >= 1.0 && height >= 1.0) {
        return Ok(None);
    }
    CANVAS.with(|cell| {
        let mut entry = cell
            .try_borrow_mut()
            .map_err(|_| Error::Jvm("NanoVG frame already active".into()))?;
        let canvas = entry
            .get_or_insert_with(|| unsafe { Canvas::create() })
            .as_ref()
            .map_err(|e| Error::Jvm(e.clone()))?;
        Ok(Frame {
            ctx: canvas.ctx,
            fonts: canvas.fonts,
            own: Some([width, height]),
            _frame: std::marker::PhantomData,
        }
        .blur_region(rect, passes))
    })
}

pub fn glow(width: f32, height: f32, passes: i32, f: impl FnOnce(&mut Frame<'_>)) -> Result<Option<i32>> {
    if !(width.is_finite() && height.is_finite() && width >= 1.0 && height >= 1.0) {
        return Ok(None);
    }
    CANVAS.with(|cell| {
        let mut entry = cell
            .try_borrow_mut()
            .map_err(|_| Error::Jvm("NanoVG frame already active".into()))?;
        let canvas = entry
            .get_or_insert_with(|| unsafe { Canvas::create() })
            .as_ref()
            .map_err(|e| Error::Jvm(e.clone()))?;
        let (w, h) = (width as c_int, height as c_int);
        let mut saved = Saved::default();
        let image = unsafe { nvgrs_layer_begin(canvas.ctx, &mut saved, w, h) };
        if image == 0 {
            return Ok(None);
        }
        struct End<'a>(&'a Canvas, Saved);
        impl Drop for End<'_> {
            fn drop(&mut self) {
                unsafe { nvgrs_layer_end(self.0.ctx, &self.1) }
            }
        }
        {
            let _end = End(canvas, saved);
            f(&mut Frame {
                ctx: canvas.ctx,
                fonts: canvas.fonts,
                own: Some([w as f32, h as f32]),
                _frame: std::marker::PhantomData,
            });
        }
        Ok((unsafe { nvgrs_layer_blur(passes) } != 0).then_some(image))
    })
}

thread_local! {
    static NATIVE_FONTS: RefCell<Vec<(usize, [c_int; 2])>> = const { RefCell::new(Vec::new()) };
}

fn native_fonts(ctx: *mut c_void) -> Result<[c_int; 2]> {
    NATIVE_FONTS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, fonts)) = cache.iter().find(|(c, _)| *c == ctx as usize) {
            return Ok(*fonts);
        }
        let font = |name: &[u8], data: &'static [u8]| unsafe {
            let found = nvgFindFont(ctx, name.as_ptr().cast());
            if found >= 0 {
                return found;
            }
            nvgCreateFontMem(ctx, name.as_ptr().cast(), data.as_ptr().cast_mut(), data.len() as c_int, 0)
        };
        let fonts = [font(b"inter\0", INTER_MEDIUM), font(b"inter-bold\0", INTER_BOLD)];
        if fonts.iter().any(|&f| f < 0) {
            return Err(Error::Jvm("native NanoVG context has no usable fonts".into()));
        }
        cache.push((ctx as usize, fonts));
        Ok(fonts)
    })
}

struct Restore(Ctx);
impl Drop for Restore {
    fn drop(&mut self) {
        unsafe { nvgRestore(self.0) }
    }
}

pub unsafe fn draw_native(ctx: *mut c_void, f: impl FnOnce(&mut Frame<'_>) -> Result<()>) -> Result<()> {
    let fonts = native_fonts(ctx)?;
    unsafe {
        nvgSave(ctx);
        nvgResetTransform(ctx);
        nvgResetScissor(ctx);
        nvgGlobalAlpha(ctx, 1.0);
    }
    let _restore = Restore(ctx);
    f(&mut Frame {
        ctx,
        fonts,
        own: None,
        _frame: std::marker::PhantomData,
    })
}

pub unsafe fn draw_in_place(ctx: *mut c_void, f: impl FnOnce(&mut Frame<'_>) -> Result<()>) -> Result<()> {
    let fonts = native_fonts(ctx)?;
    unsafe { nvgSave(ctx) };
    let _restore = Restore(ctx);
    f(&mut Frame {
        ctx,
        fonts,
        own: None,
        _frame: std::marker::PhantomData,
    })
}

pub unsafe fn translated(ctx: *mut c_void, dx: f32, dy: f32, f: impl FnOnce()) {
    unsafe {
        nvgSave(ctx);
        nvgTranslate(ctx, dx, dy);
    }
    let _restore = Restore(ctx);
    f();
}
