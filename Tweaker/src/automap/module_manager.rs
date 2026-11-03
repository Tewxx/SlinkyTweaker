pub(super) mod enable_batch;

use super::{image::Image, scan, Map};
use crate::{Error, Result};
use std::collections::BTreeSet;

pub fn resolve(image: &Image, map: &mut Map) {
    map.signature(
        image,
        "module_manager_ref",
        "55 56 53 48 83 EC 20 48 8D 6C 24 20 48 89 CE 80",
    );
    if let Some(host) = map.anchors.get("module_manager_ref") {
        let targets = scan::rip_leas(image, host.rva);
        if let Some(&manager) = targets.first().filter(|&&r| image.data(r, 0x308, true)) {
            map.insert("module_manager", manager);
        }
    }
}

pub fn validate_live(map: &Map) -> Result<()> {
    use crate::slinky::mem::{read_memory, read_u64};
    let manager = map.base + map.anchors["module_manager"].rva;
    let count = read_u64(manager + 8).unwrap_or(0) as usize;
    let ptr = read_u64(manager).unwrap_or(0) as usize;
    if !(10..=512).contains(&count) || ptr == 0 {
        return Err(Error::Jvm("automap: module manager not ready".into()));
    }
    let bytes = read_memory(ptr, count * 8)
        .ok_or_else(|| Error::Jvm("automap: live module array unreadable".into()))?;
    let known: BTreeSet<_> = map
        .modules
        .values()
        .map(|m| map.base + m.descriptor)
        .collect();
    let matched = bytes
        .chunks_exact(8)
        .filter(|b| known.contains(&(u64::from_le_bytes((*b).try_into().unwrap()) as usize)))
        .count();
    if matched < 10 || matched * 2 < count {
        return Err(Error::Jvm(
            "automap: module manager does not match descriptor catalog".into(),
        ));
    }
    for slot in 0..27 {
        let list = read_u64(manager + 0x210 + slot * 8).unwrap_or(0) as usize;
        if list == 0 {
            continue;
        }
        let mut terminated = false;
        for index in 0..256 {
            let callback = read_u64(list + index * 8)
                .ok_or_else(|| Error::Jvm("automap: unreadable event listener array".into()))?
                as usize;
            if callback == 0 {
                terminated = true;
                break;
            }
            if callback < map.base || callback >= map.base + map.image_size {
                return Err(Error::Jvm(
                    "automap: event listener outside Slinky image".into(),
                ));
            }
        }
        if !terminated {
            return Err(Error::Jvm(
                "automap: unterminated event listener array".into(),
            ));
        }
    }
    Ok(())
}
