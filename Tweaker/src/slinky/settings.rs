use super::{
    mem::{read_f32, read_flag, read_u32, write_f32},
    registration::*,
};
use crate::{automap, Error, Result};
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

type SyncFn = unsafe extern "system" fn(*mut c_void, *const c_char, *mut c_void);
type GuiFn = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void);

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Bool(bool),
    Float {
        value: f32,
        min: f32,
        max: f32,
        format: &'static str,
        step: Option<f32>,
        mult: f32,
    },
    Color([f32; 3]),
}
#[derive(Clone, Copy)]
pub(crate) struct Spec {
    pub label: &'static str,
    pub key: &'static str,
    pub kind: Kind,
}
impl Spec {
    pub const fn boolean(label: &'static str, key: &'static str, value: bool) -> Self {
        Self {
            label,
            key,
            kind: Kind::Bool(value),
        }
    }
    pub const fn float(
        label: &'static str,
        key: &'static str,
        value: f32,
        min: f32,
        max: f32,
        format: &'static str,
    ) -> Self {
        Self {
            label,
            key,
            kind: Kind::Float {
                value,
                min,
                max,
                format,
                step: None,
                mult: 1.0,
            },
        }
    }
    pub const fn step(mut self, interval: f32) -> Self {
        if let Kind::Float { ref mut step, .. } = self.kind {
            *step = Some(interval);
        }
        self
    }
    pub const fn times(mut self, factor: f32) -> Self {
        if let Kind::Float { ref mut mult, .. } = self.kind {
            *mult = factor;
        }
        self
    }
    pub const fn color(label: &'static str, key: &'static str, rgb: [f32; 3]) -> Self {
        Self {
            label,
            key,
            kind: Kind::Color(rgb),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Setting {
    descriptor: usize,
    key: usize,
    sync: SyncFn,
    gui: GuiFn,
    float: bool,
}
impl Setting {
    pub fn flag(self) -> bool {
        read_flag(self.descriptor + 0x18)
    }
    pub fn number(self, fallback: f32) -> f32 {
        read_f32(self.descriptor + 0x30)
            .filter(|v| v.is_finite())
            .unwrap_or(fallback)
    }
    pub fn color(self) -> Option<[f32; 3]> {
        let rgb = [
            read_f32(self.descriptor + 8)?,
            read_f32(self.descriptor + 0xc)?,
            read_f32(self.descriptor + 0x10)?,
        ];
        rgb.iter().all(|v| v.is_finite()).then_some(rgb)
    }
}

pub(crate) fn create<const N: usize>(base: usize, specs: [Spec; N]) -> Result<[Setting; N]> {
    create_all(base, specs)?
        .try_into()
        .map_err(|_| Error::Jvm("setting count mismatch".into()))
}

pub(crate) fn create_all(
    base: usize,
    specs: impl IntoIterator<Item = Spec>,
) -> Result<Vec<Setting>> {
    specs
        .into_iter()
        .map(|spec| {
            let (template, size, sync, gui) = match spec.kind {
                Kind::Bool(_) => {
                    automap::require(&["block_in_require_ground", "sync_bool", "gui_bool"])?;
                    (
                        automap::rva("block_in_require_ground"),
                        0x20,
                        "sync_bool",
                        "gui_bool",
                    )
                }
                Kind::Float { .. } => {
                    automap::require(&["aim_assist_hor_speed", "sync_float", "gui_float"])?;
                    (
                        automap::rva("aim_assist_hor_speed"),
                        0x40,
                        "sync_float",
                        "gui_float",
                    )
                }
                Kind::Color(_) => {
                    automap::require(&["accent_color", "sync_color", "gui_color"])?;
                    (
                        automap::rva("accent_color"),
                        0x20,
                        "sync_color",
                        "gui_color",
                    )
                }
            };
            let mut d = clone_aligned(base + template, size, spec.label)?;
            initialize(&mut d, spec)?;
            let descriptor = leak_words(d);
            let float = matches!(spec.kind, Kind::Float { .. });
            if float {
                FLOATS.lock().unwrap_or_else(|e| e.into_inner()).push(descriptor);
            }
            Ok(Setting {
                descriptor,
                float,
                key: leak_cstring(spec.key)?,
                sync: unsafe { std::mem::transmute::<usize, SyncFn>(base + automap::rva(sync)) },
                gui: unsafe { std::mem::transmute::<usize, GuiFn>(base + automap::rva(gui)) },
            })
        })
        .collect()
}

fn initialize(d: &mut [u64], spec: Spec) -> Result<()> {
    let label_at = if matches!(spec.kind, Kind::Color(_)) {
        0
    } else {
        0x10
    };
    set_usize(d, label_at, leak_cstring(spec.label)?);
    match spec.kind {
        Kind::Bool(value) => {
            set_u8(d, 0x18, value as u8);
            set_u8(d, 0x19, value as u8);
        }
        Kind::Float {
            value,
            min,
            max,
            format,
            step,
            mult,
        } => {
            set_usize(d, 0x18, leak_cstring(format)?);
            if let Some(step) = step {
                set_f32(d, 0x20, step);
            }
            set_f32(d, 0x24, mult);
            set_f32(d, 0x28, min);
            set_f32(d, 0x2c, max);
            set_f32(d, 0x30, value);
            set_f32(d, 0x34, value);
        }
        Kind::Color(rgb) => {
            for (i, v) in rgb.into_iter().enumerate() {
                set_f32(d, 0x8 + i * 4, v);
                set_f32(d, 0x14 + i * 4, v);
            }
        }
    }
    Ok(())
}

pub(crate) unsafe fn sync(settings: &[Setting], ctx: *mut c_void) {
    let floats: Vec<usize> = settings.iter().filter(|s| s.float).map(|s| s.descriptor).collect();
    unclamped_load(ctx, &floats, || {
        for s in settings {
            unsafe { (s.sync)(ctx, s.key as *const c_char, s.descriptor as *mut c_void) };
        }
    });
}

static FLOATS: Mutex<Vec<usize>> = Mutex::new(Vec::new());

pub(crate) fn is_our_float(descriptor: usize) -> bool {
    FLOATS.lock().unwrap_or_else(|e| e.into_inner()).contains(&descriptor)
}

pub(crate) fn unclamped_load(ctx: *mut c_void, floats: &[usize], load: impl FnOnce()) {
    if !UNCLAMPED.load(Ordering::Relaxed) || read_u32(ctx as usize + 8) != Some(0) {
        return load();
    }
    let ranges: Vec<_> = floats
        .iter()
        .filter_map(|&d| Some((d, read_f32(d + 0x28)?, read_f32(d + 0x2c)?)))
        .collect();
    for &(d, ..) in &ranges {
        write_f32(d + 0x28, f32::MIN);
        write_f32(d + 0x2c, f32::MAX);
    }
    load();
    for &(d, min, max) in &ranges {
        write_f32(d + 0x28, min);
        write_f32(d + 0x2c, max);
    }
}
pub(crate) static UNCLAMPED: AtomicBool = AtomicBool::new(false);
pub(crate) unsafe fn gui(settings: &[Setting], gui: *mut c_void, layout: *mut c_void) {
    for s in settings {
        unsafe { (s.gui)(layout, gui, s.descriptor as *mut c_void) };
    }
}
pub(crate) unsafe fn section(title: &CStr, gui: *mut c_void, layout: *mut c_void) {
    type HeaderFn = unsafe extern "system" fn(*mut c_void, *mut c_void, *const c_char);
    if let Ok(header) = automap::get().and_then(|m| m.address("gui_section")) {
        let header = unsafe { std::mem::transmute::<usize, HeaderFn>(header) };
        unsafe { header(gui, layout, title.as_ptr()) };
    }
}
