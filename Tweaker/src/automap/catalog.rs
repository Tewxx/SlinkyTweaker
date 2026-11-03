use super::{image::Image, Hook, Map, Module};
use std::collections::BTreeMap;

pub fn resolve(image: &Image, map: &mut Map) {
    let mut candidates: BTreeMap<String, Vec<Module>> = BTreeMap::new();
    for section in image
        .sections
        .iter()
        .filter(|s| !s.executable && s.writable)
    {
        for at in (section.range.start..section.range.end.saturating_sub(0x210)).step_by(8) {
            let Some(name) = image
                .u64(at)
                .and_then(|p| image.rva(p as usize))
                .and_then(|r| image.cstr(r))
            else {
                continue;
            };
            let Some(description) = image
                .u64(at + 8)
                .and_then(|p| image.rva(p as usize))
                .and_then(|r| image.cstr(r))
            else {
                continue;
            };
            if !(description.ends_with('.') && description.len() >= 12 && name.len() <= 48) {
                continue;
            }
            if !image.data(at, 0x210, true) {
                continue;
            }
            let category = image.u32(at + 0x10).unwrap_or(u32::MAX);
            if category > 4 || image.read(at + 0x14, 1).is_none_or(|b| b[0] > 1) {
                continue;
            }
            let mut valid = true;
            let mut hooks = Vec::new();
            for slot in 0..27 {
                let ptr = image.u64(at + 0x48 + slot * 0x10).unwrap_or(0) as usize;
                if ptr == 0 {
                    continue;
                }
                let Some(callback) = image.rva(ptr).filter(|&r| image.executable(r)) else {
                    valid = false;
                    break;
                };
                hooks.push(Hook {
                    slot,
                    callback,
                    priority: image.u32(at + 0x50 + slot * 0x10).unwrap_or(0) as i32,
                });
            }
            let callback = |offset| -> Option<Option<usize>> {
                let ptr = image.u64(at + offset)? as usize;
                if ptr == 0 {
                    Some(None)
                } else {
                    Some(Some(image.rva(ptr).filter(|&r| image.executable(r))?))
                }
            };
            let (Some(gui_callback), Some(sync_callback)) = (callback(0x1f8), callback(0x200))
            else {
                continue;
            };
            if !valid || (hooks.is_empty() && gui_callback.is_none() && sync_callback.is_none()) {
                continue;
            }
            candidates.entry(name.into()).or_default().push(Module {
                name: name.into(),
                descriptor: at,
                gui_callback,
                sync_callback,
                hooks,
                settings: BTreeMap::new(),
            });
        }
    }
    for (name, mut entries) in candidates {
        if entries.len() == 1 {
            map.modules.insert(name, entries.remove(0));
        }
    }
}
