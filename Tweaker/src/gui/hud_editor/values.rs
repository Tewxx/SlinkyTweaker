use crate::automap;
use crate::slinky::mem::{read_cstr, read_f32, read_memory, read_u32, read_u64};

fn value_address(module: &str, key: &str, kind: &str) -> Option<usize> {
    let map = automap::get().ok()?;
    let setting = map.setting(module, key).filter(|s| s.kind == kind)?;
    setting.value_address(map).ok()
}

pub(super) fn enabled(module: &str) -> bool {
    automap::get()
        .ok()
        .and_then(|map| Some(map.modules.get(module)?.enabled_address(map)))
        .and_then(|a| read_memory(a, 1))
        .is_some_and(|b| b[0] == 1)
}

pub(super) fn flags(module: &str) -> u32 {
    automap::get()
        .ok()
        .and_then(|map| read_u32(map.base + map.modules.get(module)?.descriptor + 0x18))
        .unwrap_or(0)
}

pub(super) fn flag(module: &str, key: &str, fallback: bool) -> bool {
    value_address(module, key, "bool")
        .and_then(|a| read_memory(a, 1))
        .map_or(fallback, |b| b[0] == 1)
}

pub(super) fn number(module: &str, key: &str) -> Option<f32> {
    read_f32(value_address(module, key, "float")?).filter(|v| v.is_finite())
}

pub(super) fn color(module: &str, key: &str) -> Option<[f32; 3]> {
    let a = value_address(module, key, "color")?;
    let rgb = [read_f32(a)?, read_f32(a + 4)?, read_f32(a + 8)?];
    rgb.iter().all(|v| v.is_finite()).then_some(rgb)
}

pub(super) fn option(module: &str, key: &str) -> Option<String> {
    let map = automap::get().ok()?;
    let setting = map.setting(module, key).filter(|s| s.kind == "enum")?;
    let descriptor = setting.descriptor_address(map);
    let header = read_memory(descriptor + 0x20, 0x14)?;
    let count = u32::from_le_bytes(header[..4].try_into().unwrap());
    let table = u64::from_le_bytes(header[8..16].try_into().unwrap()) as usize;
    let index = u32::from_le_bytes(header[0x10..0x14].try_into().unwrap());
    if index >= count || count > 256 {
        return None;
    }
    read_cstr(read_u64(table + index as usize * 8)? as usize, 64)
}
