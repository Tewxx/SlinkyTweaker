use crate::platform::{loaded_modules, module_path, GetCurrentProcess};
use std::ffi::c_void;
use std::ptr::null_mut;

const SLINKY_MIN_IMAGE: usize = 5 * 1024 * 1024;
const SLINKY_MAX_IMAGE: usize = 200 * 1024 * 1024;

#[repr(C)]
struct ModuleInfo {
    base: *mut c_void,
    size_of_image: u32,
    entry_point: *mut c_void,
}

#[link(name = "psapi")]
extern "system" {
    fn GetModuleInformation(
        process: *mut c_void,
        module: *mut c_void,
        info: *mut ModuleInfo,
        cb: u32,
    ) -> i32;
}

fn module_label(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn is_slinky_name(label: &str) -> bool {
    label.to_ascii_lowercase().contains("slinky")
}

struct ModuleCandidate {
    base: usize,
    size: usize,
    name: String,
}

fn enumerate_candidates() -> Vec<ModuleCandidate> {
    let process = unsafe { GetCurrentProcess() };
    let mut candidates = Vec::new();

    for module in loaded_modules() {
        let mut info = ModuleInfo {
            base: null_mut(),
            size_of_image: 0,
            entry_point: null_mut(),
        };
        let got = unsafe {
            GetModuleInformation(
                process,
                module,
                &mut info,
                std::mem::size_of::<ModuleInfo>() as u32,
            )
        };
        if got == 0 {
            continue;
        }
        let size = info.size_of_image as usize;
        if !(SLINKY_MIN_IMAGE..=SLINKY_MAX_IMAGE).contains(&size) {
            continue;
        }
        candidates.push(ModuleCandidate {
            base: module as usize,
            size,
            name: module_label(&module_path(module)),
        });
    }

    candidates
}

fn pick_best(candidates: &[ModuleCandidate]) -> Option<(usize, usize, String)> {
    candidates
        .iter()
        .max_by_key(|candidate| candidate.size)
        .map(|candidate| (candidate.base, candidate.size, candidate.name.clone()))
}

pub fn find_slinky_by_name() -> Option<(usize, usize, String)> {
    let named = enumerate_candidates()
        .into_iter()
        .filter(|candidate| is_slinky_name(&candidate.name))
        .collect::<Vec<_>>();
    pick_best(&named)
}

pub fn find_slinky_by_signature() -> Option<(usize, usize, String)> {
    let verified = enumerate_candidates()
        .into_iter()
        .filter(matches_image)
        .collect::<Vec<_>>();
    pick_best(&verified)
}

pub fn find_slinky_module() -> Option<(usize, usize, String)> {
    if let Some(found) = find_slinky_by_name() {
        if matches_image(&ModuleCandidate {
            base: found.0,
            size: found.1,
            name: found.2.clone(),
        }) {
            return Some(found);
        }
    }
    find_slinky_by_signature()
}

fn matches_image(candidate: &ModuleCandidate) -> bool {
    crate::automap::image::Image::capture(candidate.base, candidate.size)
        .is_ok_and(|image| image.is_slinky())
}
