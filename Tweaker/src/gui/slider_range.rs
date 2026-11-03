use crate::automap::{self, text_box::TextBox, Event};
use crate::slinky::mem::{read_cstr, read_f32, read_u32, read_u64, write_f32};
use crate::slinky::registration::{publish_listeners, write_ptr, SuspendedThreads};
use crate::slinky::settings::{self, UNCLAMPED};
use crate::slinky::thunk::{self, Arg};
use crate::slinky::SlinkyModule;
use crate::{Error, Result};
use std::collections::HashSet;
use std::ffi::c_void;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};

const DESCRIPTOR_SYNC: usize = 0x200;
const STEP: usize = 0x20;
const MULT: usize = 0x24;
const MIN: usize = 0x28;
const MAX: usize = 0x2c;
const VALUE: usize = 0x30;

type SyncFn = unsafe extern "system" fn(*mut c_void);
type ChangedFn = unsafe extern "system" fn(u8);

struct Runtime {
    text_box: TextBox,
    floats: HashSet<usize>,
    changed: Option<ChangedFn>,
}
static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static EDITING: Mutex<Option<usize>> = Mutex::new(None);

pub fn register(module: &SlinkyModule) -> Result<()> {
    let map = automap::get()?;
    let text_box = map
        .text_box
        .ok_or_else(|| Error::Jvm("the GUI text box is not mapped".into()))?;
    let mut floats = HashSet::new();
    let mut writes = Vec::new();
    for native in map.modules.values() {
        let descriptors: Vec<usize> = native
            .settings
            .values()
            .filter(|s| s.kind == "float")
            .map(|s| s.descriptor_address(map))
            .collect();
        if descriptors.is_empty() || native.sync_callback.is_none() {
            continue;
        }
        floats.extend(&descriptors);
        let slot = map.base + native.descriptor + DESCRIPTOR_SYNC;
        let Some(original) = read_u64(slot).map(|v| v as usize).filter(|&v| v != 0) else {
            continue;
        };
        let wrapped: &'static Wrapped = Box::leak(Box::new(Wrapped {
            original: unsafe { std::mem::transmute::<usize, SyncFn>(original) },
            floats: descriptors,
        }));
        let stub = thunk::bind(
            map.base,
            Arg::Second,
            wrapped as *const Wrapped as usize,
            sync as *const () as usize,
        )?;
        writes.push((slot, stub));
    }
    {
        let _threads = SuspendedThreads::suspend_others()?;
        for &(slot, stub) in &writes {
            write_ptr(slot, stub, "settings callback")?;
        }
    }
    RUNTIME
        .set(Runtime {
            text_box,
            floats,
            changed: map
                .address("settings_changed")
                .ok()
                .map(|a| unsafe { std::mem::transmute::<usize, ChangedFn>(a) }),
        })
        .map_err(|_| Error::Jvm("slider range runtime exists".into()))?;
    UNCLAMPED.store(true, Ordering::Relaxed);
    publish_listeners(
        module.base,
        &[(map.event(Event::UiRender)?, frame as *const () as usize, false)],
    )
}

struct Wrapped {
    original: SyncFn,
    floats: Vec<usize>,
}

unsafe extern "system" fn sync(ctx: *mut c_void, wrapped: *const Wrapped) {
    let w = unsafe { &*wrapped };
    settings::unclamped_load(ctx, &w.floats, || unsafe { (w.original)(ctx) });
}

fn is_float(r: &Runtime, descriptor: usize) -> bool {
    r.floats.contains(&descriptor) || settings::is_our_float(descriptor)
}

unsafe extern "system" fn frame(_event: *mut c_void) {
    let (Some(r), Some(gui)) = (RUNTIME.get(), crate::gui::hud_editor::gui_object()) else {
        return;
    };
    let tb = r.text_box;
    let typing = read_u32(gui + tb.state).is_some_and(|s| s & !1 == 2);
    let target = typing
        .then(|| read_u64(gui + tb.target))
        .flatten()
        .map(|t| t as usize)
        .filter(|&t| t > VALUE && is_float(r, t - VALUE));
    let mut editing = EDITING.lock().unwrap_or_else(|e| e.into_inner());
    if target.is_some() {
        *editing = target;
        return;
    }
    let Some(value) = editing.take().filter(|_| !typing) else {
        return;
    };
    if let Some(text) = read_cstr(gui + tb.text, 0x60) {
        submitted(r, value - VALUE, &text);
    }
}

fn submitted(r: &Runtime, descriptor: usize, text: &str) {
    let field = |offset| read_f32(descriptor + offset);
    let (Some(step), Some(mult), Some(min), Some(max), Some(value)) =
        (field(STEP), field(MULT), field(MIN), field(MAX), field(VALUE))
    else {
        return;
    };
    let typed = typed_value(text, step, mult);
    if !typed.is_finite() || (min < typed && typed < max) {
        return;
    }
    let clamped = if typed <= min { min } else { max };
    if value != clamped {
        return;
    }
    write_f32(descriptor + VALUE, typed);
    if let Some(changed) = r.changed {
        unsafe { changed(1) };
    }
}

fn typed_value(text: &str, step: f32, mult: f32) -> f32 {
    let bytes = text.as_bytes();
    let negative = bytes.first() == Some(&b'-');
    let mut digits = bytes[negative as usize..].iter().peekable();
    let digit = |b: Option<&&u8>| b.filter(|b| b.is_ascii_digit()).map(|b| (**b - b'0') as i32);
    let mut whole = 0i32;
    while let Some(d) = digit(digits.peek()) {
        whole = whole.wrapping_mul(10).wrapping_add(d);
        digits.next();
    }
    let sign = if negative { -1.0 } else { 1.0 };
    let mut value = if matches!(digits.peek(), Some(b'.' | b',')) {
        digits.next();
        let (mut fraction, mut place) = (0f32, 0.1f32);
        while let Some(d) = digit(digits.peek()) {
            fraction += d as f32 * place;
            place = (place as f64 * 0.1) as f32;
            digits.next();
        }
        (whole as f32 + fraction) * sign
    } else {
        (whole as f32) * sign
    };
    if mult != 0.0 {
        value /= mult;
    }
    if step != 0.0 {
        value = (value / step).round() * step;
    }
    value
}
