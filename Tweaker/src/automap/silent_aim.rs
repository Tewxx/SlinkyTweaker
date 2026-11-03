use super::{image::Image, Event, Map};

pub fn silent_index(image: &Image, map: &Map) -> Option<u32> {
    let mode = map.setting("Aim Assist", "mode")?;
    let count = image.u32(mode.descriptor + 0x20)?;
    if !(1..=16).contains(&count) {
        return None;
    }
    let pointer = image.u64(mode.descriptor + 0x18)? as usize;
    let mut at = pointer.checked_sub(image.base)?;
    for index in 0..=count {
        let mut bytes = Vec::new();
        for _ in 0..128 {
            let b = *image.read(at, 1)?.first()?;
            at += 1;
            if b == 0 {
                break;
            }
            bytes.push(b);
        }
        if index > 0 && bytes == b"Silent" {
            return Some(index - 1);
        }
    }
    None
}
pub fn resolve(image: &Image, map: &mut Map) {
    if let Some(entry) = map
        .hook("Aim Assist", Event::PlayerUpdate)
        .map(|h| h.callback)
    {
        map.native_silent_mode = silent_index(image, map);
        super::rotations::resolve_getters(image, map, entry);
    }
    for (alias, key) in [
        ("silent_aim_mode", "mode"),
        ("silent_move_correct", "silent_move_correct"),
        ("silent_ignore_manual_aim", "silent_ignore_manual_aim"),
    ] {
        map.alias_setting(alias, "Aim Assist", key);
    }
    for (name, event) in [
        ("aim_assist_silent_request", Event::SilentRequest),
        ("aim_assist_game_update", Event::GameUpdate),
    ] {
        if let Some(callback) = map.hook("Aim Assist", event).map(|h| h.callback) {
            map.insert(name, callback);
        }
    }
}
