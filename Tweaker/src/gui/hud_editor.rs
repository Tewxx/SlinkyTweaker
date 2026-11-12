mod arraylist;
mod custom_name;
mod element;
mod input;
mod layout;
mod native;
mod notifications;
mod module_edit;
mod overlay;
pub(crate) mod timewarp;
mod values;
mod visibility;

use crate::slinky::{registration::publish_listeners, SlinkyModule};
use crate::{Error, Result};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering}, OnceLock,
};

struct Runtime {
    base: usize,
    gui_enabled: usize,
    gui_scale: Option<usize>,
    gui_render: usize,
    mod_overlay_render: Option<usize>,
    notifications_render: Option<usize>,
    module_manager: Option<usize>,
    gui_object: Option<usize>,
}
static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static VISIBLE: AtomicBool = AtomicBool::new(false);
static HUD_ANIM: AtomicU32 = AtomicU32::new(0);

pub(crate) fn gui_object() -> Option<usize> {
    RUNTIME.get()?.gui_object
}

pub(crate) fn editing() -> bool {
    VISIBLE.load(Ordering::Acquire)
}

fn ui_listener(base: usize, module: &str) -> Option<usize> {
    let map = crate::automap::get().ok()?;
    let hook = map.hook(module, crate::automap::Event::UiRender)?;
    Some(base + hook.callback)
}

pub fn register(module: &SlinkyModule) -> Result<()> {
    let map = crate::automap::get()?;
    let overlay_slot = map.event(crate::automap::Event::OverlayRender)?;
    let gui = map
        .modules
        .get("GUI")
        .ok_or_else(|| Error::Jvm("HUD editor needs the mapped GUI module".into()))?;
    let gui_render = ui_listener(module.base, "GUI")
        .ok_or_else(|| Error::Jvm("GUI has no UI-pass listener".into()))?;
    let gui_object = module_edit::gui_object(gui_render);
    let runtime = Runtime {
        base: module.base,
        gui_enabled: gui.enabled_address(map),
        gui_scale: map
            .setting("GUI", "scale")
            .and_then(|s| s.value_address(map).ok()),
        gui_render,
        gui_object,
        mod_overlay_render: ui_listener(module.base, "Mod Overlay"),
        notifications_render: ui_listener(module.base, "Notifications"),
        module_manager: crate::automap::require(&["module_manager"])
            .ok()
            .map(|()| module.base + crate::automap::rva("module_manager")),
    };
    if runtime.mod_overlay_render.is_some() {
        arraylist::ELEMENT.register();
        let _ = arraylist::add_settings(module.base);
    }
    if runtime.notifications_render.is_some() {
        notifications::ELEMENT.register();
        let _ = custom_name::add_setting();
    }
    let _ = timewarp::add_style_setting(module.base);
    if let (Some(manager), Some(_)) = (runtime.module_manager, runtime.gui_object) {
        let _ = visibility::setup(manager);
    }
    let notifications_render = runtime.notifications_render;
    RUNTIME
        .set(runtime)
        .map_err(|_| Error::Jvm("HUD editor runtime exists".into()))?;
    let _ = native::take_over(module.base, "GUI", gui_render, overlay::gui_dispatch as *const () as usize);
    if let Some(render) = notifications_render {
        let ours = notifications::render as *const () as usize;
        let _ = native::take_over(module.base, "Notifications", render, ours);
    }
    publish_listeners(
        module.base,
        &[
            (overlay_slot, overlay::backdrop as *const () as usize, true),
            (overlay_slot, arraylist::capture as *const () as usize, false),
            (native::ui_slot()?, overlay::ui as *const () as usize, false),
        ],
    )
}
