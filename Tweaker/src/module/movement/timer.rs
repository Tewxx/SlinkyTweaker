use crate::automap::Event;
use crate::minecraft::game::Game;
use crate::slinky::mem::{read_flag, read_u64};
use crate::slinky::registration::{publish_listeners, write_ptr, SuspendedThreads, MODULE_ENABLED};
use crate::slinky::settings::{self as native_settings, Setting, Spec};
use crate::slinky::SlinkyModule;
use crate::{Error, Result};
use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

const TIMER: &str = "Timer";
const DESCRIPTOR_GUI: usize = 0x1f8;
const DESCRIPTOR_SYNC: usize = 0x200;
const POLL: Duration = Duration::from_millis(50);

type GuiFn = unsafe extern "system" fn(*mut c_void, *mut c_void);
type SyncFn = unsafe extern "system" fn(*mut c_void);

struct Additions {
    timer: usize,
    reset_on_input: Setting,
    gui: GuiFn,
    sync: SyncFn,
}
static ADDITIONS: OnceLock<Additions> = OnceLock::new();
static RESET_TICKS: AtomicU32 = AtomicU32::new(0);
const RESET_FOR_TICKS: u32 = 2;

fn active() -> bool {
    ADDITIONS.get().is_some_and(|a| {
        a.reset_on_input.flag()
            && read_flag(a.timer + MODULE_ENABLED)
    })
}

pub fn register(module: &SlinkyModule) -> Result<()> {
    let map = crate::automap::get()?;
    let timer = map
        .modules
        .get(TIMER)
        .ok_or_else(|| Error::Jvm("Timer is not mapped".into()))?;
    let descriptor = map.base + timer.descriptor;
    let callback = |offset| {
        read_u64(descriptor + offset)
            .map(|v| v as usize)
            .filter(|&v| v != 0)
            .ok_or_else(|| Error::Jvm("Timer has no settings callbacks".into()))
    };
    let (gui, sync) = (callback(DESCRIPTOR_GUI)?, callback(DESCRIPTOR_SYNC)?);
    let update_slot = map.event(Event::PlayerUpdate)?;
    let tick_slot = map.event(Event::ClientTick)?;
    let [reset_on_input] = native_settings::create(
        module.base,
        [Spec::boolean("Reset on input", "reset_on_input", false)],
    )?;
    ADDITIONS
        .set(Additions {
            timer: descriptor,
            reset_on_input,
            gui: unsafe { std::mem::transmute::<usize, GuiFn>(gui) },
            sync: unsafe { std::mem::transmute::<usize, SyncFn>(sync) },
        })
        .map_err(|_| Error::Jvm("Timer additions exist".into()))?;
    {
        let _threads = SuspendedThreads::suspend_others()?;
        write_ptr(descriptor + DESCRIPTOR_GUI, additions_gui as *const () as usize, "Timer GUI")?;
        write_ptr(descriptor + DESCRIPTOR_SYNC, additions_sync as *const () as usize, "Timer sync")?;
    }
    publish_listeners(
        module.base,
        &[
            (update_slot, update as *const () as usize, false),
            (tick_slot, tick as *const () as usize, false),
        ],
    )?;
    thread::Builder::new()
        .name("timer-reset-on-input".into())
        .spawn(poll)
        .map_err(|e| Error::Jvm(format!("Timer input poller: {e}")))?;
    Ok(())
}

unsafe extern "system" fn additions_gui(gui: *mut c_void, layout: *mut c_void) {
    let Some(a) = ADDITIONS.get() else {
        return;
    };
    unsafe {
        (a.gui)(gui, layout);
        native_settings::gui(&[a.reset_on_input], gui, layout);
    }
}
unsafe extern "system" fn additions_sync(ctx: *mut c_void) {
    let Some(a) = ADDITIONS.get() else {
        return;
    };
    unsafe {
        (a.sync)(ctx);
        native_settings::sync(&[a.reset_on_input], ctx);
    }
}

fn poll() {
    let mut game: Option<Game> = None;
    loop {
        thread::sleep(POLL);
        if !active() {
            RESET_TICKS.store(0, Ordering::Release);
            continue;
        }
        if game.is_none() {
            match Game::attach() {
                Ok(g) => game = Some(g),
                Err(_) => {
                    thread::sleep(Duration::from_secs(1));
                    continue;
                }
            }
        }
        let g = game.as_mut().unwrap();
        match g.timer_reset_input() {
            Ok(true) => {
                if g.set_timer_speed(1.0).is_ok() {
                    RESET_TICKS.store(RESET_FOR_TICKS, Ordering::Release);
                }
            }
            Ok(false) => {}
            Err(_) => g.clear_exception(),
        }
    }
}

thread_local! {static GAME: RefCell<Option<Game>> = const { RefCell::new(None) };}

unsafe extern "system" fn tick(_event: *mut c_void) {
    let _ = RESET_TICKS.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1));
}

unsafe extern "system" fn update(_event: *mut c_void) {
    if RESET_TICKS.load(Ordering::Acquire) == 0 || !active() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        GAME.with(|cell| {
            let Ok(mut entry) = cell.try_borrow_mut() else {
                return Ok(());
            };
            if entry.is_none() {
                *entry = Some(Game::attach()?);
            }
            entry.as_mut().unwrap().set_timer_speed(1.0)
        })
    }));
}
