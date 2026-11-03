use std::ffi::c_void;
use std::ptr::null_mut;

const MAX_MODULES: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Launcher {
    Lunar,
    Forge,
    Unknown,
}

#[link(name = "psapi")]
extern "system" {
    fn EnumProcessModules(
        process: *mut c_void,
        modules: *mut *mut c_void,
        cb: u32,
        needed: *mut u32,
    ) -> i32;
    fn GetModuleFileNameExW(
        process: *mut c_void,
        module: *mut c_void,
        lp_filename: *mut u16,
        n_size: u32,
    ) -> u32;
}

#[link(name = "kernel32")]
extern "system" {
    pub(crate) fn GetCurrentProcess() -> *mut c_void;
}

pub(crate) fn loaded_modules() -> Vec<*mut c_void> {
    let process = unsafe { GetCurrentProcess() };
    let mut handles = vec![null_mut(); MAX_MODULES];
    let mut needed = 0u32;
    let ok = unsafe {
        EnumProcessModules(
            process,
            handles.as_mut_ptr(),
            (MAX_MODULES * std::mem::size_of::<*mut c_void>()) as u32,
            &mut needed,
        )
    };
    if ok == 0 {
        return Vec::new();
    }

    let count = (needed as usize) / std::mem::size_of::<*mut c_void>();
    handles.truncate(count.min(MAX_MODULES));
    handles.retain(|module| !module.is_null());
    handles
}

pub(crate) fn module_path(module: *mut c_void) -> String {
    let mut wide = [0u16; 512];
    let len = unsafe {
        GetModuleFileNameExW(
            GetCurrentProcess(),
            module,
            wide.as_mut_ptr(),
            wide.len() as u32,
        )
    };
    if len == 0 {
        return String::new();
    }
    String::from_utf16_lossy(&wide[..len as usize])
}

pub fn detect_launcher() -> Launcher {
    let mut lunar = false;
    let mut forge = false;

    for module in loaded_modules() {
        let path = module_path(module).to_ascii_lowercase();
        if path.is_empty() {
            continue;
        }
        if path.contains("lunarclient") || path.contains("\\lunar\\") {
            lunar = true;
        }
        if path.contains("forge")
            || path.contains("launchwrapper")
            || path.contains("fml")
            || path.contains("modlauncher")
        {
            forge = true;
        }
    }

    if lunar {
        Launcher::Lunar
    } else if forge {
        Launcher::Forge
    } else {
        Launcher::Unknown
    }
}
