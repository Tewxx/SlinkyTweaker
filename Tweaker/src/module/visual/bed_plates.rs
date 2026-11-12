mod draw;
mod engine;
mod scan;
mod session;
use crate::slinky::mem::read_flag;
use crate::slinky::registration::{Catalog, ModuleSpec, MODULE_ENABLED};
use crate::slinky::SlinkyModule;
use crate::{automap::Event, Error, Result};
use std::ffi::c_void;
use std::sync::OnceLock;

const CATEGORY_VISUAL: usize = 2;

struct Runtime {
    descriptor: usize,
    settings: [crate::slinky::settings::Setting; 6],
}
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn enabled() -> bool {
    RUNTIME
        .get()
        .is_some_and(|r| read_flag(r.descriptor + MODULE_ENABLED))
}

pub fn register(module: &SlinkyModule) -> Result<()> {
    let catalog = Catalog::open(module.base, 64..=256)?;
    if catalog.contains("Bed Plates") {
        return Ok(());
    }
    let map = crate::automap::get()?;
    let event = |e| map.event(e);
    let settings = settings::create(module.base)?;
    let prepared = catalog.prepare(ModuleSpec {
        name: "Bed Plates",
        description: "Shows each bed's defense above it.",
        category: CATEGORY_VISUAL,
        gui: settings::gui,
        sync: settings::sync,
        callbacks: &[
            (event(Event::GameUpdate)?, engine::tick as *const () as usize, false),
            (event(Event::ModuleState)?, engine::on_toggle as *const () as usize, false),
            (event(Event::WorldRender)?, engine::world as *const () as usize, false),
            (event(Event::OverlayRender)?, engine::overlay as *const () as usize, false),
        ],
    })?;
    RUNTIME
        .set(Runtime {
            descriptor: prepared.descriptor,
            settings,
        })
        .map_err(|_| Error::Jvm("Bed Plates runtime exists".into()))?;
    crate::gui::hud_editor::timewarp::register(prepared.descriptor, None, settings::timewarp_extras);
    prepared.publish()
}

mod settings {
    use super::*;
    use crate::slinky::settings::{self as native, Setting, Spec};
    pub const BEDWARS_ONLY: usize = 0;
    pub const WHITELIST_OWN: usize = 1;
    pub const LAYERS: usize = 2;
    pub const SCALE: usize = 3;
    pub const BLUR: usize = 4;
    pub const HIGHLIGHT: usize = 5;

    pub(super) fn create(base: usize) -> Result<[Setting; 6]> {
        native::create(
            base,
            [
                Spec::boolean("Bedwars only", "bedwars_only", false),
                Spec::boolean("Whitelist own bed", "whitelist_own", true),
                Spec::float("Defense layers", "layers", 3.0, 1.0, 4.0, "%0.0f").step(1.0),
                Spec::float("Scale", "scale", 1.0, 0.5, 2.0, "%0.0f %%").times(100.0),
                Spec::boolean("Blur background", "blur", true),
                Spec::boolean("Highlight bed", "highlight", true),
            ],
        )
    }
    pub(super) fn timewarp_extras() -> crate::gui::hud_editor::timewarp::Extras {
        use crate::gui::hud_editor::timewarp::{Extras, Item, Widget};
        let Some(r) = RUNTIME.get() else {
            return Extras { before: None, items: Vec::new() };
        };
        let kinds = [Widget::Bool, Widget::Bool, Widget::Float, Widget::Float, Widget::Bool, Widget::Bool];
        let items = std::iter::once(Item::Section("Main".into()))
            .chain(r.settings.iter().zip(kinds).map(|(s, w)| Item::Setting(s.descriptor(), w)))
            .collect();
        Extras { before: None, items }
    }
    pub fn flag(i: usize) -> bool {
        RUNTIME.get().is_some_and(|r| r.settings[i].flag())
    }
    pub fn number(i: usize, fallback: f32) -> f32 {
        RUNTIME.get().map_or(fallback, |r| r.settings[i].number(fallback))
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
