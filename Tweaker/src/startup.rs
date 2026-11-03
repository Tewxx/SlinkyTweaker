use crate::slinky::{self, SlinkyModule};
use crate::{automap, gui, module, platform, Result};
use std::thread;
use std::time::{Duration, Instant};

type Register = fn(&SlinkyModule) -> Result<()>;

const FEATURES: &[(&str, Register)] = &[
    ("Auto Ladder", module::block::auto_ladder::register),
    ("Timer reset on input", module::movement::timer::register),
    ("Bed Plates", module::visual::bed_plates::register),
    ("Displace arrow", module::combat::displace::register),
    ("Edit HUD", gui::hud_editor::register),
    ("Slider values past range", gui::slider_range::register),
];

pub fn start() {
    let launcher = platform::detect_launcher();

    thread::spawn(move || scan_slinky(launcher));
}

fn scan_slinky(launcher: platform::Launcher) {
    let max_wait = match launcher {
        platform::Launcher::Lunar => Duration::from_secs(90),
        _ => Duration::from_secs(30),
    };
    let interval = Duration::from_secs(1);
    let deadline = Instant::now() + max_wait;
    let mut located = SlinkyModule::locate();

    while located.is_err() && Instant::now() < deadline {
        thread::sleep(interval);
        located = SlinkyModule::locate();
    }

    let Ok(module) = located else {
        return;
    };

    let failed: Vec<&str> = FEATURES
        .iter()
        .filter(|&&(_, register)| register(&module).is_err())
        .map(|&(name, _)| name)
        .collect();

    thread::sleep(Duration::from_secs(2));
    let message = match (automap::get(), failed.is_empty()) {
        (Ok(map), true) => format!("Found {} Mappings", map.anchors.len()),
        _ if failed.is_empty() => "Slinky Tweaker ready.".to_string(),
        _ => format!("Unavailable: {}.", failed.join(", ")),
    };
    let _ = slinky::notify_info(&module, &message);
}
