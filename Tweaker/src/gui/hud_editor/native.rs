use crate::slinky::mem::{read_u64, write_u64};
use crate::slinky::registration::{active_listeners, write_ptr, SuspendedThreads};
use std::ffi::c_void;

pub(super) fn ui_slot() -> crate::Result<usize> {
    crate::automap::get()?.event(crate::automap::Event::UiRender)
}

pub(super) fn ui_context(event: *mut c_void) -> Option<*mut c_void> {
    if event.is_null() {
        return None;
    }
    crate::automap::get().ok()?;
    let ui = read_u64(event as usize + 0x10)? as usize;
    let candidates = [read_u64(ui).unwrap_or(0) as usize, ui];
    if let Some(ctx) = crate::automap::ui_context::resolve(candidates) {
        return Some(ctx);
    }
    None
}

pub(super) unsafe extern "system" fn skip(_event: *mut c_void) {}

pub(super) fn replace(base: usize, candidates: &[usize], to: usize) {
    let Ok(array) = ui_slot().and_then(|slot| active_listeners(base, slot)) else {
        return;
    };
    for index in 0..256 {
        let at = array + index * 8;
        match read_u64(at).map(|v| v as usize) {
            None | Some(0) => break,
            Some(entry) if entry != to && candidates.contains(&entry) => {
                write_u64(at, to as u64);
            }
            _ => {}
        }
    }
}

pub(super) fn take_over(base: usize, module: &str, native: usize, ours: usize) -> crate::Result<()> {
    let map = crate::automap::get()?;
    let slot = ui_slot()?;
    let descriptor = map
        .modules
        .get(module)
        .ok_or_else(|| crate::Error::Jvm(format!("{module} is not mapped")))?
        .descriptor;
    {
        let _threads = SuspendedThreads::suspend_others()?;
        write_ptr(map.base + descriptor + 0x48 + slot * 0x10, ours, "UI-pass hook")?;
    }
    swap(base, native, ours, true);
    Ok(())
}

pub(super) fn swap(base: usize, native: usize, ours: usize, swapped: bool) {
    let Ok(array) = ui_slot().and_then(|slot| active_listeners(base, slot)) else {
        return;
    };
    let (from, to) = if swapped {
        (native, ours)
    } else {
        (ours, native)
    };
    for index in 0..256 {
        let at = array + index * 8;
        match read_u64(at).map(|v| v as usize) {
            None | Some(0) => break,
            Some(entry) if entry == from => {
                write_u64(at, to as u64);
            }
            _ => {}
        }
    }
}
