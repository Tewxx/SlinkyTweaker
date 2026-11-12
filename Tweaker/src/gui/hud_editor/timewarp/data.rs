use super::Widget;
use crate::automap;
use crate::slinky::mem::{read_cstr, read_f32, read_memory, read_u32, read_u64, write_f32, write_memory};
use automap::layout::{Layout, ListEdit, ModuleLayout};
use std::ffi::{c_char, CString};
use std::path::PathBuf;

pub(super) const PROFILE_TAKEN: i32 = 0x3f4;

type SetEnabledFn = unsafe extern "system" fn(usize, usize, u8) -> u8;
type SetKeybindFn = unsafe extern "system" fn(usize, usize, i32);
type ChangedFn = unsafe extern "system" fn(u8);
type LoadFn = unsafe extern "system" fn(*const c_char, usize) -> i32;
type SaveFn = unsafe extern "system" fn(*const c_char, u8, usize) -> i32;
type CountFn = unsafe extern "system" fn(usize) -> u32;
type GetFn = unsafe extern "system" fn(usize, usize) -> *const c_char;
type ColorFn = unsafe extern "system" fn(usize, usize) -> *mut f32;
type AddFn = unsafe extern "system" fn(usize, *const c_char, *const f32) -> u8;
type RemoveFn = unsafe extern "system" fn(usize, usize);
type OptionsFn = unsafe extern "system" fn(*mut *const *const c_char, *mut u64);

#[derive(Clone)]
pub(super) struct Module {
    pub descriptor: usize,
    pub name: String,
    pub description: String,
}

fn module_layout() -> Option<ModuleLayout> {
    automap::module_layout().ok()
}

fn layout() -> Option<&'static Layout> {
    automap::get().ok().map(|m| &m.layout)
}

pub(super) fn supported(widget: Widget) -> bool {
    layout().is_some_and(|l| match widget {
        Widget::Bool => l.bool.is_some(),
        Widget::Float => l.float.is_some(),
        Widget::Range => l.range.is_some(),
        Widget::Enum => l.choice.is_some(),
        Widget::Color => l.color.is_some(),
        Widget::List => l.list.is_some(),
    })
}

fn byte_set(at: usize) -> bool {
    read_memory(at, 1).is_some_and(|b| b[0] == 1)
}

impl Module {
    fn allows(&self, bit: impl Fn(&ModuleLayout) -> u32) -> bool {
        module_layout().is_some_and(|l| read_u32(self.descriptor + l.flags).is_some_and(|f| f & bit(&l) == 0))
    }
    pub fn enabled(&self) -> bool {
        module_layout().is_some_and(|l| byte_set(self.descriptor + l.enabled))
    }
    pub fn togglable(&self) -> bool {
        self.allows(|l| l.no_toggle)
    }
    pub fn bindable(&self) -> bool {
        self.allows(|l| l.no_keybind)
    }
    pub fn holdable(&self) -> bool {
        self.allows(|l| l.no_hold)
    }
    pub fn keybind(&self) -> i32 {
        module_layout()
            .and_then(|l| read_u32(self.descriptor + l.keybind))
            .map_or(0, |v| v as i32)
    }
    pub fn hold(&self) -> bool {
        module_layout().is_some_and(|l| byte_set(self.descriptor + l.hold))
    }
    pub fn gui_callback(&self) -> Option<usize> {
        let l = module_layout()?;
        read_u64(self.descriptor + l.gui).map(|v| v as usize).filter(|&v| v != 0)
    }
}

pub(super) fn modules(manager: usize, category: usize) -> Vec<Module> {
    super::super::visibility::page(manager, category)
        .into_iter()
        .filter(|(_, name)| !super::super::visibility::is_hidden(name))
        .map(|(descriptor, name)| Module {
            descriptor,
            description: module_layout()
                .and_then(|l| read_u64(descriptor + l.description))
                .and_then(|p| read_cstr(p as usize, 200))
                .unwrap_or_default(),
            name,
        })
        .collect()
}

pub(super) fn module_name(descriptor: usize) -> String {
    module_layout()
        .and_then(|l| read_u64(descriptor + l.name))
        .and_then(|p| read_cstr(p as usize, 64))
        .unwrap_or_default()
}

fn anchor(name: &str) -> Option<usize> {
    automap::get().ok()?.address(name).ok()
}

pub(super) fn settings_changed() {
    super::record::invalidate();
    if let Some(changed) = anchor("settings_changed") {
        let changed = unsafe { std::mem::transmute::<usize, ChangedFn>(changed) };
        unsafe { changed(1) };
    }
}

pub(super) fn set_enabled(manager: usize, module: &Module, enabled: bool) {
    if module.enabled() == enabled {
        return;
    }
    if let Some(set) = anchor("module_set_enabled") {
        let set = unsafe { std::mem::transmute::<usize, SetEnabledFn>(set) };
        unsafe { set(manager, module.descriptor, enabled as u8) };
    }
}

fn keybind_setter() -> Option<SetKeybindFn> {
    module_layout()?;
    let set = anchor("module_set_keybind")?;
    Some(unsafe { std::mem::transmute::<usize, SetKeybindFn>(set) })
}

pub(super) fn can_bind() -> bool {
    keybind_setter().is_some()
}

pub(super) fn set_keybind(manager: usize, module: &Module, code: i32) {
    if let Some(set) = keybind_setter() {
        unsafe { set(manager, module.descriptor, code) };
    }
}

pub(super) fn set_hold(module: &Module, hold: bool) {
    let Some(l) = module_layout() else {
        return;
    };
    if module.hold() != hold && write_memory(module.descriptor + l.hold, &[hold as u8]) {
        settings_changed();
    }
}

pub(super) struct Float {
    pub label: String,
    pub format: String,
    pub step: f32,
    pub mult: f32,
    pub min: f32,
    pub max: f32,
    pub value: f32,
}

pub(super) struct Range {
    pub label: String,
    pub format: String,
    pub step: f32,
    pub mult: f32,
    pub min: f32,
    pub max: f32,
    pub low: f32,
    pub high: f32,
}

pub(super) struct Choice {
    pub label: String,
    pub options: Vec<String>,
    pub index: usize,
    pub nullable: bool,
}

fn label_at(descriptor: usize, offset: usize) -> String {
    read_u64(descriptor + offset)
        .and_then(|p| read_cstr(p as usize, 128))
        .unwrap_or_default()
}

pub(super) fn label(descriptor: usize, widget: Widget) -> String {
    let Some(l) = layout() else {
        return String::new();
    };
    let offset = match widget {
        Widget::Bool => l.bool.map(|b| b.label),
        Widget::Float => l.float.map(|f| f.label),
        Widget::Range => l.range.map(|r| r.label),
        Widget::Enum => l.choice.map(|c| c.label),
        Widget::Color => l.color.map(|c| c.label),
        Widget::List => l.list.map(|c| c.label),
    };
    offset.map(|o| label_at(descriptor, o)).unwrap_or_default()
}

pub(super) fn flag(descriptor: usize) -> bool {
    layout().and_then(|l| l.bool).is_some_and(|b| byte_set(descriptor + b.value))
}

pub(super) fn set_flag(descriptor: usize, on: bool) {
    let Some(b) = layout().and_then(|l| l.bool) else {
        return;
    };
    if flag(descriptor) != on && write_memory(descriptor + b.value, &[on as u8]) {
        settings_changed();
    }
}

pub(super) fn float(descriptor: usize) -> Option<Float> {
    let l = layout()?.float?;
    let f = |o| read_f32(descriptor + o).filter(|v| v.is_finite());
    Some(Float {
        label: label_at(descriptor, l.label),
        format: label_at(descriptor, l.format),
        step: f(l.step).unwrap_or(0.0),
        mult: f(l.mult).filter(|&m| m != 0.0).unwrap_or(1.0),
        min: f(l.min)?,
        max: f(l.max)?,
        value: f(l.value)?,
    })
}

pub(super) fn range(descriptor: usize) -> Option<Range> {
    let l = layout()?.range?;
    let f = |o| read_f32(descriptor + o).filter(|v| v.is_finite());
    Some(Range {
        label: label_at(descriptor, l.label),
        format: label_at(descriptor, l.format),
        step: f(l.step).unwrap_or(0.0),
        mult: f(l.mult).filter(|&m| m != 0.0).unwrap_or(1.0),
        min: f(l.min)?,
        max: f(l.max)?,
        low: f(l.low)?,
        high: f(l.high)?,
    })
}

pub(super) fn set_float(descriptor: usize, value: f32) {
    let Some(l) = layout().and_then(|l| l.float) else {
        return;
    };
    if read_f32(descriptor + l.value) != Some(value) && write_f32(descriptor + l.value, value) {
        settings_changed();
    }
}

pub(super) fn set_range(descriptor: usize, low: f32, high: f32) {
    let Some(l) = layout().and_then(|l| l.range) else {
        return;
    };
    let current = (read_f32(descriptor + l.low), read_f32(descriptor + l.high));
    if current != (Some(low), Some(high)) && write_f32(descriptor + l.low, low) && write_f32(descriptor + l.high, high) {
        settings_changed();
    }
}

pub(super) fn choice(descriptor: usize) -> Option<Choice> {
    let l = layout()?.choice?;
    let count = read_u32(descriptor + l.count)? as usize;
    if count == 0 || count > 256 {
        return None;
    }
    let table = read_u64(descriptor + l.table)? as usize;
    let options = (0..count)
        .map(|i| {
            read_u64(table + i * 8)
                .and_then(|p| read_cstr(p as usize, 128))
                .unwrap_or_default()
        })
        .collect();
    Some(Choice {
        label: label_at(descriptor, l.label),
        options,
        index: read_u32(descriptor + l.value)? as usize,
        nullable: byte_set(descriptor + l.nullable),
    })
}

pub(super) fn set_choice(descriptor: usize, index: usize) {
    let Some(l) = layout().and_then(|l| l.choice) else {
        return;
    };
    if read_u32(descriptor + l.value) != Some(index as u32) && write_memory(descriptor + l.value, &(index as u32).to_le_bytes()) {
        settings_changed();
    }
}

pub(super) fn color(descriptor: usize) -> Option<[f32; 3]> {
    let at = descriptor + layout()?.color?.rgb;
    let rgb = [read_f32(at)?, read_f32(at + 4)?, read_f32(at + 8)?];
    rgb.iter().all(|v| v.is_finite()).then_some(rgb.map(|v| v.clamp(0.0, 1.0)))
}

pub(super) fn set_color(descriptor: usize, rgb: [f32; 3]) {
    if color(descriptor) == Some(rgb) {
        return;
    }
    let Some(l) = layout().and_then(|l| l.color) else {
        return;
    };
    let written = rgb
        .iter()
        .enumerate()
        .all(|(i, &v)| write_f32(descriptor + l.rgb + i * 4, v));
    if written {
        settings_changed();
    }
}

pub(super) struct List {
    pub hint: String,
    pub entries: Vec<(String, Option<[f32; 3]>)>,
    pub options: Vec<String>,
    pub colored: bool,
}

fn list_edit() -> Option<ListEdit> {
    layout()?.list?.edit
}

fn function(descriptor: usize, offset: usize) -> Option<usize> {
    read_u64(descriptor + offset).map(|p| p as usize).filter(|&p| p != 0)
}

fn storage(descriptor: usize, edit: &ListEdit) -> Option<usize> {
    read_u64(descriptor + edit.storage).map(|p| p as usize).filter(|&p| p != 0)
}

fn colored(descriptor: usize, edit: &ListEdit) -> bool {
    read_u32(descriptor + edit.mode) == Some(1) && function(descriptor, edit.color).is_some()
}

fn entry_color(descriptor: usize, edit: &ListEdit, index: usize) -> Option<usize> {
    let color = unsafe { std::mem::transmute::<usize, ColorFn>(function(descriptor, edit.color)?) };
    let ptr = unsafe { color(storage(descriptor, edit)?, index) } as usize;
    (ptr != 0).then_some(ptr)
}

fn list_options(descriptor: usize, edit: &ListEdit) -> Vec<String> {
    let Some(options) = function(descriptor, edit.options) else {
        return Vec::new();
    };
    let options = unsafe { std::mem::transmute::<usize, OptionsFn>(options) };
    let mut table: *const *const c_char = std::ptr::null();
    let mut count = 0u64;
    unsafe { options(&mut table, &mut count) };
    if table.is_null() || count > 4096 {
        return Vec::new();
    }
    (0..count as usize)
        .filter_map(|i| read_u64(table as usize + i * 8).and_then(|p| read_cstr(p as usize, 128)))
        .filter(|n| !n.is_empty())
        .collect()
}

pub(super) fn list(descriptor: usize, with_options: bool) -> Option<List> {
    let edit = list_edit()?;
    let store = storage(descriptor, &edit)?;
    let count = unsafe { std::mem::transmute::<usize, CountFn>(function(descriptor, edit.count)?) };
    let get = unsafe { std::mem::transmute::<usize, GetFn>(function(descriptor, edit.get)?) };
    let total = (unsafe { count(store) } as usize).min(1024);
    let colored = colored(descriptor, &edit);
    let entries = (0..total)
        .map(|i| {
            let name = read_cstr(unsafe { get(store, i) } as usize, 128).unwrap_or_default();
            let rgb = colored
                .then(|| entry_color(descriptor, &edit, i))
                .flatten()
                .and_then(|at| Some([read_f32(at)?, read_f32(at + 4)?, read_f32(at + 8)?]))
                .filter(|c| c.iter().all(|v| v.is_finite()))
                .map(|c| c.map(|v| v.clamp(0.0, 1.0)));
            (name, rgb)
        })
        .collect();
    Some(List {
        hint: label_at(descriptor, 8),
        entries,
        options: if with_options { list_options(descriptor, &edit) } else { Vec::new() },
        colored,
    })
}

pub(super) fn list_addable(descriptor: usize) -> bool {
    list_edit().is_some_and(|e| function(descriptor, e.options).is_some())
}

fn list_changed(descriptor: usize, edit: &ListEdit) {
    write_memory(descriptor + edit.dirty, &[1]);
    settings_changed();
}

pub(super) fn list_add(descriptor: usize, name: &str) -> bool {
    let Some(edit) = list_edit() else {
        return false;
    };
    let (Some(store), Some(add), Ok(name)) = (storage(descriptor, &edit), function(descriptor, edit.add), CString::new(name)) else {
        return false;
    };
    let add = unsafe { std::mem::transmute::<usize, AddFn>(add) };
    let rgb = [1.0f32, 0.0, 0.0];
    let added = unsafe { add(store, name.as_ptr(), rgb.as_ptr()) } != 0;
    if added {
        list_changed(descriptor, &edit);
    }
    added
}

pub(super) fn list_remove(descriptor: usize, index: usize) {
    let Some(edit) = list_edit() else {
        return;
    };
    let (Some(store), Some(remove)) = (storage(descriptor, &edit), function(descriptor, edit.remove)) else {
        return;
    };
    let remove = unsafe { std::mem::transmute::<usize, RemoveFn>(remove) };
    unsafe { remove(store, index) };
    list_changed(descriptor, &edit);
}

pub(super) fn set_list_color(descriptor: usize, index: usize, rgb: [f32; 3]) {
    let Some(edit) = list_edit().filter(|e| colored(descriptor, e)) else {
        return;
    };
    let Some(at) = entry_color(descriptor, &edit, index) else {
        return;
    };
    let current = [read_f32(at), read_f32(at + 4), read_f32(at + 8)];
    if current == rgb.map(Some) {
        return;
    }
    if rgb.iter().enumerate().all(|(i, &v)| write_f32(at + i * 4, v)) {
        list_changed(descriptor, &edit);
    }
}

pub(super) fn snap(value: f32, step: f32, min: f32, max: f32) -> f32 {
    let snapped = if step > 0.0 { (value / step).round() * step } else { value };
    snapped.clamp(min, max)
}

pub(super) fn format(format: &str, value: f32) -> String {
    format_values(if format.is_empty() { "%.2f" } else { format }, &[value])
}

pub(super) fn format_pair(format: &str, low: f32, high: f32) -> String {
    let format = if format.is_empty() { "%.2f - %.2f" } else { format };
    format_values(format, &[low, high])
}

fn format_values(format: &str, values: &[f32]) -> String {
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    let mut next = values.iter();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        if chars.peek() == Some(&'%') {
            chars.next();
            out.push('%');
            continue;
        }
        let mut precision = None;
        while let Some(&d) = chars.peek() {
            if d == '.' {
                chars.next();
                let mut digits = String::new();
                while let Some(&p) = chars.peek().filter(|p| p.is_ascii_digit()) {
                    digits.push(p);
                    chars.next();
                }
                precision = digits.parse::<usize>().ok();
            } else if d.is_ascii_digit() || d == '-' || d == '+' || d == ' ' {
                chars.next();
            } else {
                break;
            }
        }
        let value = next.next().copied().unwrap_or(0.0);
        match chars.next() {
            Some('d' | 'i') => out.push_str(&format!("{}", value.round() as i64)),
            Some(_) | None => out.push_str(&format!("{:.*}", precision.unwrap_or(6), value)),
        }
    }
    out
}

pub(super) fn parse_number(text: &str) -> Option<f32> {
    let cleaned: String = text
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '.' | ','))
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    cleaned.parse::<f32>().ok().filter(|v| v.is_finite())
}

const KEY_NAMES: [&str; 104] = [
    "NONE", "MOUSE1", "MOUSE2", "MOUSE3", "MOUSE4", "MOUSE5", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
    "NUMPAD0", "NUMPAD1", "NUMPAD2", "NUMPAD3", "NUMPAD4", "NUMPAD5", "NUMPAD6", "NUMPAD7", "NUMPAD8", "NUMPAD9",
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T", "U", "V",
    "W", "X", "Y", "Z", "COLON", "PLUS", "COMMA", "MINUS", "PERIOD", "SLASH", "GRAVE", "LBRACKET", "BACKSLASH",
    "RBRACKET", "QUOTE", "LEFT", "UP", "RIGHT", "DOWN", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10",
    "F11", "F12", "F13", "F14", "F15", "F16", "BACKSPACE", "TAB", "ENTER", "CAPS", "ESC", "SPACE", "PREV", "NEXT",
    "END", "HOME", "PRNTSCR", "INS", "DEL", "NUMLOCK", "SCROLL", "LSHIFT", "RSHIFT", "LCONTROL", "RCONTROL", "LMENU",
    "RMENU",
];

pub(super) fn key_name(code: i32) -> &'static str {
    usize::try_from(code)
        .ok()
        .and_then(|c| KEY_NAMES.get(c))
        .copied()
        .unwrap_or("NONE")
}

pub(super) fn key_code(vk: usize, extended: bool) -> i32 {
    match vk {
        0x30..=0x39 => 6 + (vk - 0x30) as i32,
        0x60..=0x69 => 16 + (vk - 0x60) as i32,
        0x41..=0x5a => 26 + (vk - 0x41) as i32,
        0x70..=0x7f => 67 + (vk - 0x70) as i32,
        0xba => 52,
        0xbb => 53,
        0xbc => 54,
        0xbd => 55,
        0xbe => 56,
        0xbf => 57,
        0xc0 => 58,
        0xdb => 59,
        0xdc => 60,
        0xdd => 61,
        0xde => 62,
        0x25 => 63,
        0x26 => 64,
        0x27 => 65,
        0x28 => 66,
        0x08 => 83,
        0x09 => 84,
        0x0d => 85,
        0x14 => 86,
        0x1b => 87,
        0x20 => 88,
        0x21 => 89,
        0x22 => 90,
        0x23 => 91,
        0x24 => 92,
        0x2c => 93,
        0x2d => 94,
        0x2e => 95,
        0x90 => 96,
        0x91 => 97,
        0x10 | 0xa0 => 98,
        0xa1 => 99,
        0x11 | 0xa2 => if extended { 101 } else { 100 },
        0xa3 => 101,
        0x12 | 0xa4 => if extended { 103 } else { 102 },
        0xa5 => 103,
        _ => 0,
    }
}

fn profiles_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("USERPROFILE")?).join(".slinky").join("profiles"))
}

pub(super) fn profiles() -> Vec<String> {
    let Some(entries) = profiles_dir().and_then(|d| std::fs::read_dir(d).ok()) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| !n.ends_with(".tmp"))
        .collect();
    names.sort_by_key(|n| n.to_ascii_lowercase());
    names
}

pub(super) fn can_load_profiles() -> bool {
    anchor("profile_load").is_some()
}

pub(super) fn can_save_profiles() -> bool {
    anchor("profile_save").is_some()
}

pub(super) fn load_profile(manager: usize, name: Option<&str>) -> Option<i32> {
    let load = unsafe { std::mem::transmute::<usize, LoadFn>(anchor("profile_load")?) };
    let name = match name {
        Some(n) => Some(CString::new(n).ok()?),
        None => None,
    };
    let ptr = name.as_ref().map_or(std::ptr::null(), |n| n.as_ptr());
    let result = unsafe { load(ptr, manager) };
    super::record::invalidate();
    Some(result)
}

pub(super) fn save_profile(manager: usize, name: &str, overwrite: bool) -> Option<i32> {
    let save = unsafe { std::mem::transmute::<usize, SaveFn>(anchor("profile_save")?) };
    let name = CString::new(name).ok()?;
    Some(unsafe { save(name.as_ptr(), overwrite as u8, manager) })
}

pub(super) fn delete_profile(name: &str) -> bool {
    let Some(dir) = profiles_dir() else {
        return false;
    };
    let path = dir.join(name);
    path.parent() == Some(dir.as_path()) && std::fs::remove_file(path).is_ok()
}

pub(super) fn valid_profile_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && name.len() <= 32
        && !name.starts_with('.')
        && !name.chars().any(|c| matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control())
}
