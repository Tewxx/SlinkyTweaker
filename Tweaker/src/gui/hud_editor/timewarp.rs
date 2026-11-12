mod account;
mod data;
mod record;
mod view;

pub(crate) use record::{register, Extras, Item, Widget};
pub(super) use view::{Input, Timewarp};

use crate::slinky::mem::read_u64;
use crate::slinky::registration::{write_ptr, SuspendedThreads};
use crate::slinky::settings::{self as native_settings, Setting, Spec};
use crate::{automap, Error, Result};
use std::ffi::c_void;
use std::sync::OnceLock;

const MODULE: &str = "GUI";
const STYLES: &[&str] = &["Slinky", "Timewarp"];
const TIMEWARP: u32 = 1;

type GuiFn = unsafe extern "system" fn(*mut c_void, *mut c_void);
type SyncFn = unsafe extern "system" fn(*mut c_void);

struct Style {
    setting: Setting,
    gui: GuiFn,
    sync: SyncFn,
}
static STYLE: OnceLock<Style> = OnceLock::new();

pub(super) fn add_style_setting(base: usize) -> Result<()> {
    let map = automap::get()?;
    let module = map
        .modules
        .get(MODULE)
        .ok_or_else(|| Error::Jvm("GUI is not mapped".into()))?;
    let descriptor = map.base + module.descriptor;
    let callback = |offset| {
        read_u64(descriptor + offset)
            .map(|v| v as usize)
            .filter(|&v| v != 0)
            .ok_or_else(|| Error::Jvm("GUI has no settings callbacks".into()))
    };
    let (gui_slot, sync_slot) = automap::callback_slots()?;
    let (gui, sync) = (callback(gui_slot)?, callback(sync_slot)?);
    let [setting] = native_settings::create(base, [Spec::choice("GUI Style", "gui_style", STYLES, 0)])?;
    STYLE
        .set(Style {
            setting,
            gui: unsafe { std::mem::transmute::<usize, GuiFn>(gui) },
            sync: unsafe { std::mem::transmute::<usize, SyncFn>(sync) },
        })
        .map_err(|_| Error::Jvm("GUI style setting exists".into()))?;
    register(descriptor, Some(gui), style_extras);
    let _threads = SuspendedThreads::suspend_others()?;
    write_ptr(descriptor + gui_slot, style_gui as *const () as usize, "GUI settings")?;
    write_ptr(descriptor + sync_slot, style_sync as *const () as usize, "GUI sync")
}

unsafe extern "system" fn style_gui(gui: *mut c_void, layout: *mut c_void) {
    let Some(style) = STYLE.get() else {
        return;
    };
    unsafe {
        (style.gui)(gui, layout);
        native_settings::section(c"Style", gui, layout);
        native_settings::gui(&[style.setting], gui, layout);
    }
}

unsafe extern "system" fn style_sync(ctx: *mut c_void) {
    let Some(style) = STYLE.get() else {
        return;
    };
    unsafe {
        (style.sync)(ctx);
        native_settings::sync(&[style.setting], ctx);
    }
}

pub(super) fn close(manager: usize) {
    let Ok(map) = automap::get() else {
        return;
    };
    if let Some(module) = map.modules.get(MODULE) {
        let descriptor = map.base + module.descriptor;
        let gui = data::Module { descriptor, name: MODULE.into(), description: String::new() };
        data::set_enabled(manager, &gui, false);
    }
}

pub(super) fn active() -> bool {
    STYLE.get().is_some_and(|s| s.setting.index() == TIMEWARP)
}

fn style_extras() -> Extras {
    let items = STYLE
        .get()
        .map(|s| vec![Item::Section("Style".into()), Item::Setting(s.setting.descriptor(), Widget::Enum)])
        .unwrap_or_default();
    Extras { before: None, items }
}

pub(super) fn hidden(module: usize, item: &Item) -> bool {
    let Item::Setting(d, widget) = *item else {
        return false;
    };
    let gui = automap::get()
        .ok()
        .and_then(|map| Some(map.base + map.modules.get(MODULE)?.descriptor));
    if gui != Some(module) {
        return false;
    }
    let label = data::label(d, widget).to_ascii_lowercase();
    let options: Vec<String> = data::choice(d)
        .map(|c| c.options.iter().map(|o| o.to_ascii_lowercase()).collect())
        .unwrap_or_default();
    label == "layout" || label == "compact" || label == "wide" || (options.iter().any(|o| o == "compact") && options.iter().any(|o| o == "wide"))
}
