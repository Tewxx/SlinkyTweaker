use super::draw::{self, View};
use super::scan::{self, Defense, Scanner, RANGE};
use super::session::{self, Bedwars, Session, OWN_BED_RADIUS};
use super::{enabled, settings, RUNTIME};
use crate::minecraft::world::{Section, World};
use crate::minecraft::game::Game;
use crate::slinky::mem::{read_f32, read_memory, read_u64};
use crate::Result;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static DEFENSES: Mutex<Vec<Defense>> = Mutex::new(Vec::new());
static VIEW: Mutex<Option<View>> = Mutex::new(None);
static RESET: AtomicBool = AtomicBool::new(false);
const BLUR_PASSES: i32 = 3;

#[derive(Default)]
struct State {
    world: usize,
    scanner: Scanner,
    session: Session,
    retry_after: Option<Instant>,
}
thread_local! {
    static GAME: RefCell<Option<(Game, State)>> = const { RefCell::new(None) };
}

fn publish(defenses: Vec<Defense>) {
    *DEFENSES.lock().unwrap_or_else(|e| e.into_inner()) = defenses;
}
fn callback(_what: &str, f: impl FnOnce(&mut Game, &mut State) -> Result<()>) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        GAME.with(|cell| {
            let Ok(mut entry) = cell.try_borrow_mut() else {
                return Ok(());
            };
            if entry.is_none() {
                *entry = Some((Game::attach()?, State::default()));
            }
            let (game, state) = entry.as_mut().unwrap();
            if state.retry_after.is_some_and(|at| Instant::now() < at) {
                return Ok(());
            }
            let result = f(game, state);
            if result.is_err() {
                state.retry_after = Some(Instant::now() + Duration::from_secs(1));
                game.clear_exception();
            }
            result
        })
    }));
}

fn clear(game: &mut Game, state: &mut State) {
    if state.world != 0 {
        game.delete_global(state.world as _);
    }
    *state = State::default();
    publish(Vec::new());
}

pub(super) unsafe extern "system" fn tick(_ctx: *mut c_void) {
    let reset = RESET.swap(false, Ordering::AcqRel);
    if !reset && !enabled() {
        return;
    }
    callback("scan", |game, state| {
        if reset {
            clear(game, state);
        }
        if !enabled() {
            return Ok(());
        }
        let world_before = state.world;
        let found = game.with_world(|world| {
            let Some(world) = world else {
                return Ok(None);
            };
            if !world.is(world_before as _) {
                return Ok(Some(None));
            }
            update(world, state).map(|d| Some(Some(d)))
        })?;
        match found {
            None => publish(Vec::new()),
            Some(None) => {
                clear(game, state);
                state.world = game.with_world(|world| Ok(world.map_or(0, |w| w.global() as usize)))?;
            }
            Some(Some(defenses)) => publish(defenses),
        }
        Ok(())
    });
}

fn update(world: &mut World<'_>, state: &mut State) -> Result<Vec<Defense>> {
    if state.session.due() {
        let read = world
            .sidebar()?
            .map(|(title, lines)| Bedwars::read(&title, &lines))
            .unwrap_or_default();
        state.session.update(read);
    }
    if state.session.take_scan() {
        state.session.own_bed = own_bed(world)?;
    }
    if settings::flag(settings::BEDWARS_ONLY) && !state.session.bedwars.game {
        return Ok(Vec::new());
    }
    let p = world.position;
    let chunk = ((p[0].floor() as i32) >> 4, (p[2].floor() as i32) >> 4);
    state.scanner.tick(chunk, |cx, cz| {
        let mut beds = Vec::new();
        for sy in 0..16 {
            if let Some(states) = world.section(cx, sy, cz)? {
                beds.extend(scan::section_beds(&states, cx, sy, cz));
            }
        }
        Ok::<_, crate::Error>(beds)
    })?;

    let whitelist = settings::flag(settings::WHITELIST_OWN);
    let layers = settings::number(settings::LAYERS, 3.0).round() as i32;
    let mut sections = Sections::default();
    let mut defenses = Vec::new();
    for &bed in state.scanner.beds() {
        let c = bed.centre();
        let distance: f64 = (0..3).map(|i| (c[i] - p[i]).powi(2)).sum();
        if distance > RANGE * RANGE {
            continue;
        }
        if whitelist && bed.blocks().iter().any(|&b| state.session.is_own_bed(b)) {
            continue;
        }
        let defense = scan::defense(bed, layers, |b| sections.state(world, b));
        sections.check()?;
        defenses.push(defense);
    }
    Ok(defenses)
}

#[derive(Default)]
struct Sections {
    cache: HashMap<(i32, i32, i32), Option<Section>>,
    error: Option<crate::Error>,
}
impl Sections {
    fn state(&mut self, world: &mut World<'_>, [x, y, z]: [i32; 3]) -> u16 {
        if !(0..256).contains(&y) || self.error.is_some() {
            return 0;
        }
        let key = (x >> 4, y >> 4, z >> 4);
        let section = match self.cache.get(&key) {
            Some(section) => section,
            None => {
                let read = world.section(key.0, key.1, key.2).unwrap_or_else(|e| {
                    self.error = Some(e);
                    None
                });
                self.cache.entry(key).or_insert(read)
            }
        };
        section
            .as_ref()
            .map_or(0, |s| s[((y & 15) << 8 | (z & 15) << 4 | (x & 15)) as usize])
    }
    fn check(&mut self) -> Result<()> {
        self.error.take().map_or(Ok(()), Err)
    }
}

fn own_bed(world: &mut World<'_>) -> Result<Option<[[i32; 3]; 2]>> {
    let [px, py, pz] = world.position.map(|v| v.floor() as i32);
    let r = OWN_BED_RADIUS;
    let mut beds = Vec::new();
    for cx in (px - r) >> 4..=(px + r) >> 4 {
        for cz in (pz - r) >> 4..=(pz + r) >> 4 {
            for sy in ((py - r).max(0) >> 4)..=((py + r).min(255) >> 4) {
                let Some(states) = world.section(cx, sy, cz)? else {
                    continue;
                };
                for (i, &state) in states.iter().enumerate() {
                    if state >> 4 != scan::BED {
                        continue;
                    }
                    let b = scan::position(cx, sy, cz, i);
                    if (b[0] - px).abs() <= r && (b[1] - py).abs() <= r && (b[2] - pz).abs() <= r {
                        beds.push((b, state & 8 != 0));
                    }
                }
            }
        }
    }
    Ok(session::nearest_bed(&beds, [px, py, pz]))
}

pub(super) unsafe extern "system" fn world(event: *mut c_void) {
    if event.is_null() || !enabled() {
        return;
    }
    let Some(bytes) = read_memory(event as usize + 8, 24) else {
        return;
    };
    let origin: [f64; 3] = std::array::from_fn(|i| f64::from_le_bytes(bytes[i * 8..][..8].try_into().unwrap()));
    if !origin.iter().all(|v| v.is_finite()) {
        return;
    }
    unsafe {
        *VIEW.lock().unwrap_or_else(|e| e.into_inner()) = draw::capture(origin);
        if settings::flag(settings::HIGHLIGHT) {
            draw::highlight(&DEFENSES.lock().unwrap_or_else(|e| e.into_inner()), origin);
        }
    }
}

pub(super) unsafe extern "system" fn overlay(event: *mut c_void) {
    if event.is_null() || !enabled() {
        return;
    }
    let size = [0x28, 0x2c].map(|at| read_f32(event as usize + at).unwrap_or(0.0));
    if !size.iter().all(|v| v.is_finite() && *v > 0.0 && *v < 65536.0) {
        return;
    }
    let Some(view) = *VIEW.lock().unwrap_or_else(|e| e.into_inner()) else {
        return;
    };
    let defenses = DEFENSES.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let plates = draw::plates(&view, &defenses, size);
    if plates.is_empty() {
        return;
    }
    let scale = settings::number(settings::SCALE, 1.0).clamp(0.5, 2.0);
    let blur = settings::flag(settings::BLUR);
    let mut icons = Vec::new();
    let drawn = crate::gui::nanovg::draw(size[0], size[1], |vg| {
        let image = if blur { vg.blur(BLUR_PASSES) } else { None };
        icons = draw::pills(vg, &plates, scale, image);
        Ok(())
    });
    if drawn.is_err() {
        return;
    }
    callback("icons", |game, _| game.draw_block_icons(&icons, size));
}

pub(super) unsafe extern "system" fn on_toggle(event: *mut c_void) {
    let Some(r) = RUNTIME.get() else { return };
    if !event.is_null()
        && read_u64(event as usize + 0x10) == Some(r.descriptor as u64)
        && read_memory(event as usize + 0x18, 1).as_deref() == Some(&[0])
    {
        RESET.store(true, Ordering::Release);
        publish(Vec::new());
    }
}
