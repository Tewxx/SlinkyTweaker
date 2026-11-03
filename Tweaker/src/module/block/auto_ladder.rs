use crate::automap::Event;
use crate::slinky::aim::Aim;
use crate::slinky::blocks::{
    add, face_centre, full, offset, quarter, replaceable, scaffold, HORIZONTALS, LADDER, UP, VINE,
};
use crate::slinky::game::{Game, Natives};
use crate::slinky::hotbar::Hotbar;
use crate::slinky::mem::{read_flag, read_memory, read_u64};
use crate::minecraft::game::Game as Minecraft;
use crate::slinky::place::{aim_point, ray_at, Placer, REACH};
use crate::slinky::registration::{Catalog, ModuleSpec, MODULE_ENABLED};
use crate::slinky::SlinkyModule;
use crate::{Error, Result};
use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

const SILENT_ROTATION_PRIORITY: u32 = 900;
const PENDING_TICKS: u64 = 2;
const PLACED_TICKS: u64 = 20;

struct Runtime {
    descriptor: usize,
    settings: [crate::slinky::settings::Setting; 2],
    aim: Aim,
    natives: Natives,
    hotbar: Hotbar,
    block_in: Option<usize>,
}
static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static SILENT: AtomicBool = AtomicBool::new(false);

fn enabled() -> bool {
    RUNTIME
        .get()
        .is_some_and(|r| read_flag(r.descriptor + MODULE_ENABLED))
}
fn block_in_active() -> bool {
    RUNTIME
        .get()
        .and_then(|r| r.block_in)
        .is_some_and(read_flag)
}

pub fn register(module: &SlinkyModule) -> Result<()> {
    let map = crate::automap::get()?;
    let aim = Aim::resolve(map, SILENT_ROTATION_PRIORITY, 30.0, 10.0)?;
    let natives = Natives::resolve(map)?;
    let hotbar = Hotbar::resolve(map)?;
    let event = |e| map.event(e);
    let catalog = Catalog::open(module.base, 64..=256)?;
    if catalog.contains("Auto Ladder") {
        return Ok(());
    }
    let settings = settings::create(module.base)?;
    let prepared = catalog.prepare(ModuleSpec {
        name: "Auto Ladder",
        description: "Places a ladder to catch your fall.",
        category: 3,
        gui: settings::gui,
        sync: settings::sync,
        callbacks: &[
            (event(Event::SilentRequest)?, request_silent as *const () as usize, false),
            (event(Event::PlayerUpdate)?, aim_tick as *const () as usize, false),
            (event(Event::GameUpdate)?, update as *const () as usize, false),
            (event(Event::ClientTick)?, tick as *const () as usize, false),
            (event(Event::ModuleState)?, on_toggle as *const () as usize, false),
        ],
    })?;
    RUNTIME
        .set(Runtime {
            descriptor: prepared.descriptor,
            settings,
            aim,
            natives,
            hotbar,
            block_in: map.modules.get("Block In").map(|m| m.enabled_address(map)),
        })
        .map_err(|_| Error::Jvm("Auto Ladder runtime exists".into()))?;
    prepared.publish()
}

mod settings {
    use super::*;
    use crate::slinky::settings::{self as native, Setting, Spec};
    pub const MIN_FALL: usize = 0;
    pub const HOLDING_LADDER: usize = 1;

    pub(super) fn create(base: usize) -> Result<[Setting; 2]> {
        native::create(
            base,
            [
                Spec::float("Minimum fall distance", "min_fall", 5.0, 5.0, 40.0, "%0.1f blocks")
                    .step(0.5),
                Spec::boolean("Only while holding ladder", "only_holding_ladder", true),
            ],
        )
    }
    pub fn number(i: usize) -> f32 {
        RUNTIME.get().map_or(5.0, |r| r.settings[i].number(5.0))
    }
    pub fn flag(i: usize) -> bool {
        RUNTIME.get().is_some_and(|r| r.settings[i].flag())
    }
    pub(super) unsafe extern "system" fn sync(ctx: *mut c_void) {
        if let Some(r) = RUNTIME.get() {
            unsafe { native::sync(&r.settings, ctx) };
        }
    }
    pub(super) unsafe extern "system" fn gui(gui: *mut c_void, layout: *mut c_void) {
        if let Some(r) = RUNTIME.get() {
            unsafe {
                native::section(c"Main", gui, layout);
                native::gui(&r.settings, gui, layout);
            }
        }
    }
}

fn hotbar_slots(g: &Game) -> Result<(Option<i32>, Option<i32>)> {
    let (mut block, mut most, mut ladder) = (None, 0, None);
    for slot in 0..9 {
        let Some((id, size)) = g.hotbar_block(slot)? else {
            continue;
        };
        if scaffold(id) {
            if size > most {
                block = Some(slot);
                most = size;
            }
        } else if id == LADDER {
            ladder = Some(slot);
        }
    }
    Ok((block, ladder))
}

fn block_id(g: &Game, at: [i32; 3]) -> Result<i32> {
    Ok(g.block_id_at(at)?.unwrap_or(0))
}

#[derive(Clone, Copy, PartialEq)]
struct Plan {
    support: [i32; 3],
    side: i32,
    ladder: bool,
    toward: i32,
}
impl Plan {
    fn ladder(wall: [i32; 3], toward: i32) -> Self {
        Plan { support: wall, side: toward ^ 1, ladder: true, toward }
    }
    fn spot(&self) -> [i32; 3] {
        if self.ladder {
            offset(self.support, self.side)
        } else {
            offset(offset(self.support, self.toward ^ 1), UP)
        }
    }
    fn valid(&self, g: &Game) -> Result<bool> {
        let id = block_id(g, self.support)?;
        Ok(!replaceable(id)
            && (!self.ladder || full(id))
            && replaceable(block_id(g, offset(self.support, self.side))?))
    }
}

fn plan(g: &Game, yaw: f32, ground: [i32; 3], landing: [f64; 3]) -> Result<Option<Plan>> {
    let spot = offset(ground, UP);
    let facing = quarter(yaw);
    let order = [1, 3, 0, 2].map(|turn| HORIZONTALS[((facing + turn) & 3) as usize]);
    for side in order {
        let wall = offset(spot, side);
        let id = block_id(g, wall)?;
        if !replaceable(id) && full(id) {
            return Ok(Some(Plan::ladder(wall, side)));
        }
    }
    let [x, _, z] = landing;
    let (m, m1) = (0.415, 1.415);
    for side in order {
        let beside = offset(ground, side);
        let above = offset(beside, UP);
        let (ax, az) = (above[0] as f64, above[2] as f64);
        let inside = x > ax - m && x < ax + m1 && z > az - m && z < az + m1;
        if !inside && !replaceable(block_id(g, beside)?) && replaceable(block_id(g, above)?) {
            return Ok(Some(Plan { support: beside, side: UP, ladder: false, toward: side }));
        }
    }
    Ok(None)
}

#[derive(Default)]
struct State {
    placer: Option<Placer>,
    ticks: u64,
    placed_at: Option<u64>,
    pending: Option<([i32; 3], u64)>,
    plan: Option<Plan>,
    committed: bool,
    queued: Option<Click>,
    aim: Option<[f64; 3]>,
    silent: bool,
    tick_eye: Option<[f64; 3]>,
    lead: [f64; 3],
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state(f: impl FnOnce(&Runtime, &mut State) -> Result<()>) {
    let Some(r) = RUNTIME.get() else { return };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Ok(mut state) = STATE.try_lock() else {
            return Ok(());
        };
        let state = state.get_or_insert_with(State::default);
        if state.placer.is_none() {
            state.placer = Some(Placer::new(r.hotbar));
        }
        f(r, state)
    }));
    if result.is_err() {
        SILENT.store(false, Ordering::Release);
    }
}

fn placing(state: &State) -> bool {
    state.placer.as_ref().is_some_and(Placer::active)
}

fn forget(state: &mut State) {
    state.pending = None;
    state.plan = None;
    state.committed = false;
    state.queued = None;
    state.aim = None;
}

fn reset(g: &Game, state: &mut State) -> Result<()> {
    forget(state);
    match state.placer.as_mut() {
        Some(placer) => placer.release(g, true),
        None => Ok(()),
    }
}

unsafe extern "system" fn update(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    with_state(|r, state| {
        let on = enabled();
        if !on && !placing(state) && state.aim.is_none() {
            return Ok(());
        }
        let ran = unsafe {
            r.natives.frame(ctx, |g| {
                if on {
                    game_update(g, state)
                } else {
                    reset(g, state)
                }
            })
        }?;
        if ran.is_none() {
            forget(state);
        }
        Ok(())
    });
}

fn game_update(g: &Game, state: &mut State) -> Result<()> {
    state.ticks += 1;
    let player = g.player();
    if !g.focused()? || block_in_active() {
        return reset(g, state);
    }
    let fall = g.fall_distance(player)?;
    let position = g.position(player)?;
    let feet = position.map(|v| v.floor() as i32);
    let on_ladder = matches!(block_id(g, feet)?, LADDER | VINE);
    let on_ground = g.on_ground(player)?;
    if on_ground || on_ladder || fall < 1.5 {
        state.tick_eye = None;
        return reset(g, state);
    }
    let eye = g.eye()?;
    state.lead = state.tick_eye.map_or([0.0; 3], |last| next_move(last, eye));
    state.tick_eye = Some(eye);
    falling(g, state, fall)
}

fn next_move(last: [f64; 3], eye: [f64; 3]) -> [f64; 3] {
    let [dx, dy, dz] = [eye[0] - last[0], eye[1] - last[1], eye[2] - last[2]];
    [dx * 0.91, (dy - 0.08) * 0.98, dz * 0.91]
}

#[derive(Clone, Copy)]
struct Click {
    block: [i32; 3],
    side: i32,
    hit: [f64; 3],
}

thread_local! {
    static MINECRAFT: RefCell<Option<Minecraft>> = const { RefCell::new(None) };
}

fn right_click(click: Click) -> Result<bool> {
    MINECRAFT.with(|cell| {
        let mut entry = cell
            .try_borrow_mut()
            .map_err(|_| Error::Jvm("Auto Ladder's game handle is busy".into()))?;
        if entry.is_none() {
            *entry = Some(Minecraft::attach()?);
        }
        entry
            .as_mut()
            .unwrap()
            .right_click_block(click.block, click.side, click.hit)
    })
}

unsafe extern "system" fn tick(_event: *mut c_void) {
    with_state(|_, state| {
        let Some(click) = state.queued.take() else {
            return Ok(());
        };
        right_click(click)?;
        Ok(())
    });
}

fn ground_below(g: &Game, from: [f64; 3]) -> Result<Option<[i32; 3]>> {
    let hit = ray_at(g, from, 0.0, 90.0, 7.0)?;
    Ok(hit.filter(|h| h.side == UP).map(|h| h.block))
}

fn landing_point(feet: [f64; 3], mut step: [f64; 3], top: f64) -> [f64; 3] {
    let mut at = feet;
    for _ in 0..40 {
        if at[1] < top {
            break;
        }
        at = add(at, step, 1.0);
        step = [step[0] * 0.91, (step[1] - 0.08) * 0.98, step[2] * 0.91];
    }
    at
}

fn falling(g: &Game, state: &mut State, fall: f32) -> Result<()> {
    if state.placed_at.is_some_and(|at| state.ticks < at + PLACED_TICKS) {
        return reset(g, state);
    }
    let (Some(block_slot), Some(ladder_slot)) = hotbar_slots(g)? else {
        return reset(g, state);
    };
    if settings::flag(settings::HOLDING_LADDER) && !placing(state) && g.current_slot()? != ladder_slot
    {
        return reset(g, state);
    }
    if fall < settings::number(settings::MIN_FALL) - 1.5 {
        return reset(g, state);
    }
    let (eye, angles) = (g.eye()?, g.angles()?);
    let Some(below) = ground_below(g, eye)? else {
        return reset(g, state);
    };
    let feet = g.position(g.player())?;
    let top = below[1] as f64 + 2.0;
    let landing = landing_point(feet, state.lead, top);
    let ground = match ground_below(g, [landing[0], eye[1], landing[2]])? {
        Some(ground) => ground,
        None => below,
    };
    if !replaceable(block_id(g, offset(ground, UP))?) {
        return reset(g, state);
    }
    let spot = offset(ground, UP);
    let kept = match state.plan {
        Some(p) if p.valid(g)? && (state.committed || p.spot() == spot) => Some(p),
        _ => None,
    };
    let next = match kept {
        Some(p) => Some(p),
        None => plan(g, angles.yaw, ground, landing)?,
    };
    let Some(plan) = next else {
        return reset(g, state);
    };
    state.plan = Some(plan);
    let point = aim_point(g, eye, plan.support, plan.side)?;
    state.aim = Some(point.unwrap_or_else(|| face_centre(plan.support, plan.side)));
    if ray_at(g, eye, angles.yaw, 90.0, REACH)?.is_none() {
        return Ok(());
    }
    let target = offset(plan.support, plan.side);
    if state.pending.is_some_and(|(at, until)| at == target && state.ticks < until) {
        return Ok(());
    }
    let slot = if plan.ladder { ladder_slot } else { block_slot };
    let placer = state.placer.as_mut().expect("placer is created with the state");
    if !placer.hold(g, slot)? {
        return Ok(());
    }
    let hit = state.aim.unwrap_or_else(|| face_centre(plan.support, plan.side));
    state.queued = Some(Click { block: plan.support, side: plan.side, hit });
    state.pending = Some((target, state.ticks + PENDING_TICKS));
    if plan.ladder {
        state.placed_at = Some(state.ticks);
    } else {
        state.plan = Some(Plan::ladder(target, plan.toward));
        state.committed = true;
    }
    Ok(())
}

unsafe extern "system" fn aim_tick(ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    with_state(|r, state| {
        if !enabled() {
            state.silent = false;
            SILENT.store(false, Ordering::Release);
            return Ok(());
        }
        let Some(update) = r.aim.update(ctx)? else {
            return Ok(());
        };
        let claimed = r.aim.claim(state.aim.is_some(), &mut state.silent);
        SILENT.store(state.silent, Ordering::Release);
        if !claimed {
            return Ok(());
        }
        let Some(eye) = (unsafe { r.natives.frame(ctx, |g| g.eye()) })? else {
            state.silent = false;
            SILENT.store(false, Ordering::Release);
            return Ok(());
        };
        let eye = if state.tick_eye == Some(eye) {
            add(eye, state.lead, 1.0)
        } else {
            eye
        };
        let result = r.aim.aim_at(&update, eye, state.aim, &mut state.silent);
        SILENT.store(state.silent, Ordering::Release);
        result.map(|_| ())
    });
}

unsafe extern "system" fn request_silent(event: *mut c_void) {
    if event.is_null() || !enabled() || !SILENT.load(Ordering::Acquire) {
        return;
    }
    let Some(r) = RUNTIME.get() else { return };
    r.aim.request_silent(event);
}

unsafe extern "system" fn on_toggle(event: *mut c_void) {
    let Some(r) = RUNTIME.get() else { return };
    if !event.is_null()
        && read_u64(event as usize + 0x10) == Some(r.descriptor as u64)
        && read_memory(event as usize + 0x18, 1).as_deref() == Some(&[0])
    {
        with_state(|_, state| {
            state.placed_at = None;
            Ok(())
        });
    }
}
