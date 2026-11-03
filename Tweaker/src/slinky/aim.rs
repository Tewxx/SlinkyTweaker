use super::mem::{read_memory, read_u64, write_memory};
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use crate::{
    automap::{jni::EventLayout, Map},
    Error, Result,
};
use std::ffi::c_void;

pub const REQUIRED: &[&str] = &[
    "rotation_claim",
    "rotation_add",
    "rotation_current",
    "rotation_stored",
    "camera_override_active",
    "packed_angle_add",
    "packed_angle_sub",
    "native_aim_apply",
    "native_angles_to",
    "native_angle_magnitude",
    "native_wrap_yaw",
    "jni_player_angles",
];

type Claim = unsafe extern "system" fn(u32) -> u8;
type Flag = unsafe extern "system" fn() -> u8;
type Packed = unsafe extern "system" fn() -> u64;
type Apply = unsafe extern "system" fn(u64, f32) -> u64;
type Add = unsafe extern "system" fn(u64);
type Binary = unsafe extern "system" fn(u64, u64) -> u64;
type ToAngles = unsafe extern "system" fn(*const [f64; 3], *const [f64; 3]) -> u64;
type Magnitude = unsafe extern "system" fn(u64) -> f32;
type Wrap = unsafe extern "system" fn(f32) -> f32;
type PlayerAngles = unsafe extern "system" fn(*mut c_void, *mut c_void) -> u64;

pub fn unpack(angles: u64) -> (f32, f32) {
    (
        f32::from_bits(angles as u32),
        f32::from_bits((angles >> 32) as u32),
    )
}

pub(crate) struct Aim {
    layout: EventLayout,
    claim: Claim,
    active: Flag,
    current: Packed,
    stored: Packed,
    apply: Apply,
    add: Add,
    aim_settings: [usize; 5],
    silent_mode: u32,
    packed_add: Binary,
    packed_sub: Binary,
    angles_to: ToAngles,
    magnitude: Magnitude,
    wrap: Wrap,
    player_angles: PlayerAngles,
    priority: u32,
    speed: f32,
    return_threshold: f32,
}

pub(crate) struct Update {
    event: usize,
    dt: f32,
}

impl Aim {
    pub fn resolve(map: &Map, priority: u32, speed: f32, return_threshold: f32) -> Result<Self> {
        map.require(REQUIRED)?;
        let layout = map
            .event_layout
            .ok_or_else(|| Error::Jvm("rotation event layout not mapped".into()))?;
        let f = |name| map.address(name);
        let setting = |key, kind| -> Result<usize> {
            let setting = map
                .setting("Aim Assist", key)
                .filter(|s| s.kind == kind)
                .ok_or_else(|| Error::Jvm(format!("Aim Assist setting {key}/{kind} unavailable")))?;
            let address = setting.value_address(map)?;
            if kind != "bool" && address % 4 != 0 {
                return Err(Error::Jvm(format!("unaligned Aim Assist setting {key}")));
            }
            Ok(address)
        };
        unsafe {
            Ok(Self {
                layout,
                claim: std::mem::transmute(f("rotation_claim")?),
                active: std::mem::transmute(f("camera_override_active")?),
                current: std::mem::transmute(f("rotation_current")?),
                stored: std::mem::transmute(f("rotation_stored")?),
                apply: std::mem::transmute(f("native_aim_apply")?),
                add: std::mem::transmute(f("rotation_add")?),
                aim_settings: [
                    setting("mode", "enum")?,
                    setting("hor_speed", "float")?,
                    setting("ver_speed", "float")?,
                    setting("fov", "range")?,
                    setting("require_mouse_moved", "bool")?,
                ],
                silent_mode: map
                    .native_silent_mode
                    .ok_or_else(|| Error::Jvm("Aim Assist Silent mode not mapped".into()))?,
                packed_add: std::mem::transmute(f("packed_angle_add")?),
                packed_sub: std::mem::transmute(f("packed_angle_sub")?),
                angles_to: std::mem::transmute(f("native_angles_to")?),
                magnitude: std::mem::transmute(f("native_angle_magnitude")?),
                wrap: std::mem::transmute(f("native_wrap_yaw")?),
                player_angles: std::mem::transmute(f("jni_player_angles")?),
                priority,
                speed,
                return_threshold,
            })
        }
    }

    pub fn update(&self, context: *mut c_void) -> Result<Option<Update>> {
        if context.is_null() {
            return Ok(None);
        }
        let event = read_u64(context as usize + self.layout.context_event).unwrap_or(0) as usize;
        if event == 0 || read_memory(event + self.layout.phase, 1).as_deref() != Some(&[1]) {
            return Ok(None);
        }
        let dt = read_memory(context as usize + self.layout.player_elapsed, 8)
            .map(|b| f64::from_le_bytes(b.try_into().unwrap()) as f32)
            .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0)
            .ok_or_else(|| Error::Jvm("invalid rotation update elapsed time".into()))?;
        Ok(Some(Update { event, dt }))
    }

    pub fn claim(&self, has_target: bool, silent: &mut bool) -> bool {
        unsafe {
            if has_target {
                if (self.claim)(self.priority) == 0 {
                    return false;
                }
                *silent = true;
                (self.active)() != 0
            } else {
                if !*silent {
                    return false;
                }
                if (self.active)() == 0 || (self.claim)(self.priority) == 0 {
                    *silent = false;
                    return false;
                }
                true
            }
        }
    }

    pub fn aim_at(
        &self,
        update: &Update,
        eye: [f64; 3],
        target: Option<[f64; 3]>,
        silent: &mut bool,
    ) -> Result<f32> {
        let view = self.view_packed(self.player(update.event)?);
        let goal = match target {
            Some(point) => unsafe { (self.angles_to)(&eye, &point) },
            None => unsafe { (self.stored)() },
        };
        let diff = self.difference(goal, view);
        let error = unsafe { (self.magnitude)(diff) };
        if target.is_none() && error < self.return_threshold {
            *silent = false;
        }
        let (yaw, pitch) = unpack(diff);
        if !(yaw.is_finite() && pitch.is_finite()) {
            return Err(Error::Jvm("aim angles not finite".into()));
        }
        if unsafe { (self.claim)(self.priority) } != 0 {
            if target.is_some() {
                unsafe { (self.add)(diff) };
            } else {
                self.step(diff, update.dt);
            }
        }
        Ok(error)
    }

    fn step(&self, diff: u64, dt: f32) {
        let [mode, hor, ver, fov, mouse] = self.aim_settings;
        unsafe {
            let _mode = Swap32::new(mode, self.silent_mode);
            let _hor = Swap32::new(hor, self.speed.to_bits());
            let _ver = Swap32::new(ver, self.speed.to_bits());
            let _fov = Swap32::new(fov, 0.0f32.to_bits());
            let _mouse = Swap8::new(mouse, 0);
            (self.apply)(diff, dt);
        }
    }

    pub fn request_silent(&self, event: *mut c_void) {
        if event.is_null() {
            return;
        }
        let _ = write_memory(event as usize + self.layout.silent_requested, &[1]);
        let _ = write_memory(event as usize + self.layout.silent_move_correct, &[1]);
    }

    fn view_packed(&self, player_angles: u64) -> u64 {
        unsafe { (self.packed_add)(player_angles, (self.current)()) }
    }
    fn difference(&self, target: u64, current: u64) -> u64 {
        let value = unsafe { (self.packed_sub)(target, current) };
        let yaw = unsafe { (self.wrap)(f32::from_bits(value as u32)) };
        (value & 0xffff_ffff_0000_0000) | yaw.to_bits() as u64
    }
    fn player(&self, event: usize) -> Result<u64> {
        let head = read_u64(event + self.layout.jni_head).unwrap_or(0);
        let player = read_u64(event + self.layout.player).unwrap_or(0);
        if head == 0 || player == 0 {
            return Err(Error::Jvm("player rotation context unavailable".into()));
        }
        let angles = unsafe { (self.player_angles)(head as *mut c_void, player as *mut c_void) };
        let (yaw, pitch) = unpack(angles);
        if yaw.is_finite() && pitch.is_finite() {
            Ok(angles)
        } else {
            Err(Error::Jvm("player angles not finite".into()))
        }
    }
}

struct Swap32 {
    address: usize,
    old: u32,
    value: u32,
}
impl Swap32 {
    unsafe fn new(address: usize, value: u32) -> Self {
        let old = unsafe { (*(address as *const AtomicU32)).swap(value, Ordering::SeqCst) };
        Self { address, old, value }
    }
}
impl Drop for Swap32 {
    fn drop(&mut self) {
        let cell = unsafe { &*(self.address as *const AtomicU32) };
        let _ = cell.compare_exchange(self.value, self.old, Ordering::SeqCst, Ordering::SeqCst);
    }
}
struct Swap8 {
    address: usize,
    old: u8,
    value: u8,
}
impl Swap8 {
    unsafe fn new(address: usize, value: u8) -> Self {
        let old = unsafe { (*(address as *const AtomicU8)).swap(value, Ordering::SeqCst) };
        Self { address, old, value }
    }
}
impl Drop for Swap8 {
    fn drop(&mut self) {
        let cell = unsafe { &*(self.address as *const AtomicU8) };
        let _ = cell.compare_exchange(self.value, self.old, Ordering::SeqCst, Ordering::SeqCst);
    }
}
