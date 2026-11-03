use super::arrow::{self, wrap_degrees, Arrow};
use super::{RENDER_ARROW, RUNTIME};
use crate::slinky::mem::{read_flag, read_u32};
use crate::slinky::game::{Binding, Game};
use crate::slinky::mem::read_memory;
use crate::Result;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const RANDOM: u32 = 2;
const MIN_PUSH: f32 = 45.0;

static CANCELLED_BEFORE: AtomicU8 = AtomicU8::new(0);
static NUDGE: AtomicBool = AtomicBool::new(false);
static ARROW: Mutex<Option<Arrow>> = Mutex::new(None);

fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

fn enabled() -> bool {
    RUNTIME.get().is_some_and(|r| read_flag(r.native.enabled))
}
pub(super) unsafe extern "system" fn before_click(event: *mut c_void) {
    if let Some(flags) = (!event.is_null()).then(|| read_memory(event as usize + 0x10, 2)).flatten() {
        CANCELLED_BEFORE.store(flags[1], Ordering::Release);
    }
}

pub(super) unsafe extern "system" fn after_click(event: *mut c_void) {
    if event.is_null() {
        return;
    }
    if let Some(flags) = read_memory(event as usize + 0x10, 2) {
        if enabled() && flags[0] == 1 && flags[1] == 1 && CANCELLED_BEFORE.load(Ordering::Acquire) == 0 {
            NUDGE.store(true, Ordering::Release);
        }
    }
}

pub(super) unsafe extern "system" fn tick(ctx: *mut c_void) {
    let Some(r) = RUNTIME.get() else { return };
    let nudge = NUDGE.swap(false, Ordering::AcqRel);
    if ctx.is_null() || !enabled() || !r.settings[RENDER_ARROW].flag() {
        clear_arrow();
        return;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        r.natives.frame(ctx, aim)
    }));
    let aim = match result {
        Ok(Ok(aim)) => aim.flatten(),
        _ => None,
    };
    ARROW
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(Arrow::default)
        .update(aim, nudge, now());
}

fn aim(g: &Game) -> Result<Option<(arrow::Target, f32)>> {
    let Some(r) = RUNTIME.get() else { return Ok(None) };
    if !g.key_pressed(g.key(Binding::Attack)?)? || read_u32(r.native.direction) == Some(RANDOM) {
        return Ok(None);
    }
    let Some(target) = g.crosshair_player()? else {
        return Ok(None);
    };
    if g.tagged(target)? || !conditions_met(g)? {
        return Ok(None);
    }
    let push = g.displace_yaw(target)?;
    if push.abs() < MIN_PUSH {
        return Ok(None);
    }
    let bounds = g.bounds(target)?;
    let target = arrow::Target {
        id: g.entity_id(target)?,
        position: g.position(target)?,
        prev_position: g.previous_position(target)?,
        half_width: (bounds.max[0] - bounds.min[0]) * 0.5,
    };
    Ok(Some((target, wrap_degrees(g.angles()?.yaw + push))))
}

fn clear_arrow() {
    *ARROW.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn conditions_met(g: &Game) -> Result<bool> {
    let Some(r) = RUNTIME.get() else { return Ok(false) };
    let (weapon, knockback) = g.holding_weapon()?;
    if read_flag(r.native.require_weapon) && !weapon {
        return Ok(false);
    }
    Ok(knockback || (!read_flag(r.native.require_kb_enchant) && g.sprinting()?))
}

pub(super) unsafe extern "system" fn render(event: *mut c_void) {
    let Some(r) = RUNTIME.get() else { return };
    if event.is_null() || !enabled() || !r.settings[RENDER_ARROW].flag() {
        return;
    }
    let Some(bytes) = read_memory(event as usize + 8, 28) else {
        return;
    };
    let origin: [f64; 3] = std::array::from_fn(|i| f64::from_le_bytes(bytes[i * 8..][..8].try_into().unwrap()));
    if !origin.iter().all(|v| v.is_finite()) {
        return;
    }
    let partial = f32::from_le_bytes(bytes[24..28].try_into().unwrap());
    let partial = if (0.0..=1.0).contains(&partial) { partial } else { 1.0 };
    if let Some(arrow) = ARROW.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        unsafe { arrow.render(origin, partial, crate::slinky::accent_color(), now()) };
    }
}
