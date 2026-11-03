use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

struct Store {
    anchors: BTreeMap<String, [f32; 2]>,
    dirty: bool,
}
static STORE: Mutex<Option<Store>> = Mutex::new(None);

fn path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join("slinkytweaker").join("hud_layout.txt"))
}

fn parse(text: &str) -> BTreeMap<String, [f32; 2]> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let id = parts.next()?;
            let x: f32 = parts.next()?.parse().ok()?;
            let y: f32 = parts.next()?.parse().ok()?;
            (x.is_finite() && y.is_finite()).then(|| (id.to_string(), [x, y]))
        })
        .collect()
}

fn with<T>(f: impl FnOnce(&mut Store) -> T) -> T {
    let mut store = STORE.lock().unwrap_or_else(|e| e.into_inner());
    let store = store.get_or_insert_with(|| Store {
        anchors: path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|text| parse(&text))
            .unwrap_or_default(),
        dirty: false,
    });
    f(store)
}

pub(super) fn get(id: &str) -> Option<[f32; 2]> {
    with(|s| s.anchors.get(id).copied())
}
pub(super) fn set(id: &str, anchor: [f32; 2]) {
    with(|s| {
        if s.anchors.insert(id.to_string(), anchor) != Some(anchor) {
            s.dirty = true;
        }
    });
}
pub(super) fn remove(id: &str) {
    with(|s| {
        if s.anchors.remove(id).is_some() {
            s.dirty = true;
        }
    });
}

pub(super) fn save() {
    let text = with(|s| {
        if !s.dirty {
            return None;
        }
        s.dirty = false;
        Some(
            s.anchors
                .iter()
                .map(|(id, [x, y])| format!("{id} {x} {y}\n"))
                .collect::<String>(),
        )
    });
    let (Some(text), Some(path)) = (text, path()) else {
        return;
    };
    let written = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, text));
    let _ = written;
}
