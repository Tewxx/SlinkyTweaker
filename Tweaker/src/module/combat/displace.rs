mod arrow;
mod engine;
use crate::slinky::game::Natives;
use crate::slinky::mem::read_u64;
use crate::slinky::registration::{publish_listeners, write_ptr, SuspendedThreads};
use crate::slinky::sections;
use crate::slinky::settings::{self as native_settings, Setting, Spec};
use crate::slinky::SlinkyModule;
use crate::{automap, automap::Event, Error, Result};
use std::ffi::c_void;
use std::sync::OnceLock;

const MODULE: &str = "Knockback Displace";

type GuiFn = unsafe extern "system" fn(*mut c_void, *mut c_void);
type SyncFn = unsafe extern "system" fn(*mut c_void);

struct Native {
    enabled: usize,
    direction: usize,
    require_weapon: usize,
    require_kb_enchant: usize,
}

struct Runtime {
    native: Native,
    natives: Natives,
    settings: [Setting; 1],
    sectioned: bool,
    gui: GuiFn,
    sync: SyncFn,
}
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

const RENDER_ARROW: usize = 0;

pub fn register(module: &SlinkyModule) -> Result<()> {
    let map = automap::get()?;
    map.require(automap::displace::REQUIRED)?;
    let natives = Natives::resolve(map)?;
    let native = map
        .modules
        .get(MODULE)
        .ok_or_else(|| Error::Jvm("Knockback Displace is not mapped".into()))?;
    let value = |key: &str, kind: &str| {
        map.setting(MODULE, key)
            .filter(|s| s.kind == kind)
            .ok_or_else(|| Error::Jvm(format!("Knockback Displace {key} is not mapped")))?
            .value_address(map)
    };
    let direction = map
        .setting(MODULE, "direction")
        .filter(|s| s.kind == "enum")
        .ok_or_else(|| Error::Jvm("Knockback Displace direction is not mapped".into()))?
        .descriptor_address(map);
    let native_state = Native {
        enabled: native.enabled_address(map),
        direction: direction + 0x30,
        require_weapon: value("require_weapon", "bool")?,
        require_kb_enchant: value("require_kb_enchant", "bool")?,
    };
    let descriptor = map.base + native.descriptor;
    let callback = |offset| {
        read_u64(descriptor + offset)
            .map(|v| v as usize)
            .filter(|&v| v != 0)
            .ok_or_else(|| Error::Jvm("Knockback Displace has no settings callbacks".into()))
    };
    let (gui_slot, sync_slot) = automap::callback_slots()?;
    let (gui, sync) = (callback(gui_slot)?, callback(sync_slot)?);
    let settings = native_settings::create(
        module.base,
        [
            Spec::boolean("Render arrow", "render_arrow", true),
        ],
    )?;
    let native_gui = gui;
    let customization =
        sections::insert_before(map, MODULE, gui, "Condition", c"Customization", &settings);
    let (gui, sectioned) = match customization {
        Ok(copy) => (copy, true),
        Err(_) => (gui, false),
    };
    RUNTIME
        .set(Runtime {
            native: native_state,
            natives,
            settings,
            sectioned,
            gui: unsafe { std::mem::transmute::<usize, GuiFn>(gui) },
            sync: unsafe { std::mem::transmute::<usize, SyncFn>(sync) },
        })
        .map_err(|_| Error::Jvm("Knockback Displace additions exist".into()))?;
    crate::gui::hud_editor::timewarp::register(descriptor, Some(native_gui), timewarp_extras);
    let click = map.event(Event::ClickPre)?;
    publish_listeners(
        module.base,
        &[
            (click, engine::before_click as *const () as usize, true),
            (click, engine::after_click as *const () as usize, false),
            (map.event(Event::GameUpdate)?, engine::tick as *const () as usize, false),
            (map.event(Event::WorldRender)?, engine::render as *const () as usize, false),
        ],
    )?;
    let _threads = SuspendedThreads::suspend_others()?;
    write_ptr(descriptor + gui_slot, gui_wrapper as *const () as usize, "Displace GUI")?;
    write_ptr(descriptor + sync_slot, sync_wrapper as *const () as usize, "Displace sync")
}

fn timewarp_extras() -> crate::gui::hud_editor::timewarp::Extras {
    use crate::gui::hud_editor::timewarp::{Extras, Item, Widget};
    let Some(r) = RUNTIME.get() else {
        return Extras { before: None, items: Vec::new() };
    };
    let settings = r.settings.iter().map(|s| Item::Setting(s.descriptor(), Widget::Bool));
    if r.sectioned {
        Extras {
            before: Some("Condition"),
            items: std::iter::once(Item::Section("Customization".into())).chain(settings).collect(),
        }
    } else {
        Extras { before: None, items: settings.collect() }
    }
}

unsafe extern "system" fn gui_wrapper(gui: *mut c_void, layout: *mut c_void) {
    let Some(r) = RUNTIME.get() else {
        return;
    };
    unsafe {
        (r.gui)(gui, layout);
        if !r.sectioned {
            native_settings::gui(&r.settings, gui, layout);
        }
    }
}
unsafe extern "system" fn sync_wrapper(ctx: *mut c_void) {
    let Some(r) = RUNTIME.get() else {
        return;
    };
    unsafe {
        (r.sync)(ctx);
        native_settings::sync(&r.settings, ctx);
    }
}
