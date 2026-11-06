use crate::slinky::{self, SlinkyModule};
use crate::{automap, gui, module, platform, Error, Result};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

type Register = fn(&SlinkyModule) -> Result<()>;

const FEATURES: &[(&str, Register)] = &[
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

    let module = match located {
        Ok(module) => module,
        Err(error) => return report_startup_failure(&error),
    };

    let failed: Vec<(&str, String)> = FEATURES
        .iter()
        .filter_map(|&(name, register)| {
            match catch_unwind(AssertUnwindSafe(|| register(&module))) {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some((name, error.to_string())),
                Err(_) => Some((name, "panicked during registration".to_string())),
            }
        })
        .collect();

    let mut log = vec![format!("Slinky base {:#x}", module.base)];
    if let Ok(map) = automap::get() {
        log.push(format!(
            "{} anchors, {} modules, {} events, manager layout {:x?}",
            map.anchors.len(),
            map.modules.len(),
            map.events.len(),
            map.manager_layout,
        ));
    }
    log.extend(failed.iter().map(|(name, error)| format!("{name}: {error}")));
    write_log(&log);

    thread::sleep(Duration::from_secs(2));
    let names: Vec<&str> = failed.iter().map(|&(name, _)| name).collect();
    let message = match (automap::get(), names.len()) {
        (Ok(map), 0) => format!("Found {} Mappings", map.anchors.len()),
        (_, 0) => "Slinky Tweaker ready.".to_string(),
        (_, count) if count == FEATURES.len() => {
            "All tweaker features unavailable. See .slinky\\tweaker.log".to_string()
        }
        _ => format!("Unavailable: {}.", names.join(", ")),
    };
    if let Err(error) = slinky::notify_info(&message) {
        log.push(format!("notification failed: {error}"));
        write_log(&log);
    }
}

fn report_startup_failure(error: &Error) {
    let mut log = vec![format!("Startup failed: {error}")];
    let summary = error.to_string();
    let summary = summary.lines().next().unwrap_or_default();
    let message = format!("Slinky Tweaker failed: {summary}");

    let Some((base, size, name)) = automap::locate::find_slinky_module() else {
        log.push("no loaded module matched Slinky".to_string());
        return write_log(&log);
    };
    log.push(format!("Slinky module {name} base {base:#x} size {size:#x}"));
    let shown = automap::image::Image::capture(base, size)
        .and_then(|image| automap::resolve(&image))
        .and_then(|map| {
            log.push(format!(
                "{} anchors, {} modules, {} events",
                map.anchors.len(),
                map.modules.len(),
                map.events.len(),
            ));
            slinky::notify_with_map(&map, &message)
        });
    if let Err(error) = shown {
        log.push(format!("notification failed: {error}"));
    }
    write_log(&log);
}

fn log_path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("USERPROFILE")?).join(".slinky").join("tweaker.log"))
}

fn write_log(lines: &[String]) {
    if let Some(path) = log_path() {
        let _ = std::fs::write(path, lines.join("\n") + "\n");
    }
}
