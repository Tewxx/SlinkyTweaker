use super::RUNTIME;
use crate::slinky::mem::{read_u64, write_u64};
use crate::slinky::registration::{module_title, read_pointer_array};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub(super) const CATEGORIES: [&str; 5] = ["Combat", "Move", "Visual", "Block", "Utility"];

struct Flags {
    names: Vec<String>,
    hidden: Vec<AtomicBool>,
}
static FLAGS: OnceLock<Flags> = OnceLock::new();
static GENERATION: AtomicU64 = AtomicU64::new(0);
static SAVE: Mutex<()> = Mutex::new(());

fn key(name: &str) -> String {
    let words: Vec<String> = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    format!("hide_{}", words.join("_"))
}

fn saved_true(text: &str, key: &str) -> bool {
    let quoted = format!("\"{key}\"");
    text.match_indices(&quoted).any(|(at, _)| {
        let rest = text[at + quoted.len()..].trim_start();
        rest.strip_prefix(':').is_some_and(|v| v.trim_start().starts_with("true"))
    })
}

pub(super) fn slinky_file(name: &str) -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("USERPROFILE")?).join(".slinky").join(name))
}

fn file_text(names: &[String], hidden: impl Fn(usize) -> bool) -> String {
    let lines: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(i, name)| format!("  \"{}\": {}", key(name), hidden(i)))
        .collect();
    format!("{{\n{}\n}}\n", lines.join(",\n"))
}

fn save(flags: &Flags) {
    let Some(path) = slinky_file("hidden_settings") else {
        return;
    };
    let _guard = SAVE.lock().unwrap_or_else(|e| e.into_inner());
    let text = file_text(&flags.names, |i| flags.hidden[i].load(Ordering::Relaxed));
    let temporary = path.with_extension("tmp");
    let _ = std::fs::write(&temporary, text).and_then(|()| std::fs::rename(&temporary, &path));
}

pub(super) fn setup(manager: usize) -> crate::Result<()> {
    let names: Vec<String> = (0..CATEGORIES.len())
        .flat_map(|category| page(manager, category))
        .map(|(_, name)| name)
        .collect();
    let saved = slinky_file("hidden_settings")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    let hidden = names.iter().map(|name| AtomicBool::new(saved_true(&saved, &key(name)))).collect();
    FLAGS
        .set(Flags { names, hidden })
        .map_err(|_| crate::Error::Jvm("hidden module flags exist".into()))
}

fn flag(name: &str) -> Option<&'static AtomicBool> {
    let flags = FLAGS.get()?;
    flags.names.iter().position(|n| n == name).map(|i| &flags.hidden[i])
}

pub(super) fn is_hidden(name: &str) -> bool {
    flag(name).is_some_and(|f| f.load(Ordering::Relaxed))
}

pub(super) fn ready() -> bool {
    FLAGS.get().is_some()
}

pub(super) fn set_hidden(name: &str, hidden: bool) {
    let (Some(flags), Some(flag)) = (FLAGS.get(), flag(name)) else {
        return;
    };
    if flag.swap(hidden, Ordering::Relaxed) != hidden {
        GENERATION.fetch_add(1, Ordering::Relaxed);
        save(flags);
    }
}

fn signature() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

fn category_at(manager: usize, category: usize) -> Option<usize> {
    let layout = crate::automap::module_manager::layout().ok()?;
    Some(manager + layout.categories + category * 0x10)
}

#[derive(Clone, Copy, Default)]
struct Filtered {
    source: (usize, usize),
    signature: u64,
    array: usize,
    count: usize,
}

thread_local! {
    static NAMES: RefCell<HashMap<usize, String>> = RefCell::new(HashMap::new());
    static FILTERED: RefCell<[Filtered; 5]> = RefCell::new([Filtered::default(); 5]);
}

pub(super) fn name(descriptor: usize) -> Option<String> {
    NAMES.with(|names| {
        let mut names = names.borrow_mut();
        if let Some(name) = names.get(&descriptor) {
            return Some(name.clone());
        }
        let name = module_title(descriptor)?;
        names.insert(descriptor, name.clone());
        Some(name)
    })
}

pub(super) fn page(manager: usize, category: usize) -> Vec<(usize, String)> {
    let Some(at) = category_at(manager, category) else {
        return Vec::new();
    };
    let (Some(array), Some(count)) = (read_u64(at), read_u64(at + 8)) else {
        return Vec::new();
    };
    if count > 1024 {
        return Vec::new();
    }
    read_pointer_array(array as usize, count as usize, "category")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|d| name(d).map(|n| (d, n)))
        .collect()
}

fn filtered(manager: usize, category: usize, source: (usize, usize), signature: u64) -> Option<(usize, usize)> {
    FILTERED.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entry = &mut cache[category];
        if entry.source != source || entry.signature != signature || entry.array == 0 {
            let modules = page(manager, category);
            if modules.len() != source.1 {
                return None;
            }
            let shown: Vec<usize> = modules
                .into_iter()
                .filter(|(_, n)| !is_hidden(n))
                .map(|(d, _)| d)
                .collect();
            let count = shown.len();
            let mut words = shown;
            words.push(0);
            let array = Box::leak(words.into_boxed_slice()).as_ptr() as usize;
            *entry = Filtered { source, signature, array, count };
        }
        (entry.count < source.1).then_some((entry.array, entry.count))
    })
}

pub(super) unsafe extern "system" fn gui_render(event: *mut c_void) {
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    let native: unsafe extern "system" fn(*mut c_void) = unsafe { std::mem::transmute(runtime.gui_render) };
    let Some(manager) = runtime.module_manager else {
        return unsafe { native(event) };
    };
    let signature = signature();
    let mut swapped = Vec::new();
    for category in 0..CATEGORIES.len() {
        let Some(at) = category_at(manager, category) else {
            continue;
        };
        let (Some(array), Some(count)) = (read_u64(at), read_u64(at + 8)) else {
            continue;
        };
        let source = (array as usize, count as usize);
        if let Some((array, count)) = filtered(manager, category, source, signature) {
            write_u64(at + 8, count as u64);
            write_u64(at, array as u64);
            swapped.push((at, source));
        }
    }
    unsafe { native(event) };
    for (at, (array, count)) in swapped {
        write_u64(at, array as u64);
        write_u64(at + 8, count as u64);
    }
}
