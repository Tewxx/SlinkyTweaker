use crate::{Error, Result};
use std::ffi::c_void;
use std::sync::Mutex;

#[derive(Clone, Copy)]
pub(crate) enum Arg {
    Second,
    Fourth,
}

pub(crate) fn bind(near: usize, arg: Arg, value: usize, target: usize) -> Result<usize> {
    let mut code = match arg {
        Arg::Second => vec![0x48, 0xBA],
        Arg::Fourth => vec![0x49, 0xB9],
    };
    code.extend(value.to_le_bytes());
    code.extend([0x48, 0xB8]);
    code.extend(target.to_le_bytes());
    code.extend([0xFF, 0xE0]);
    let size = code.len().next_multiple_of(16);

    let mut pages = PAGES.lock().unwrap_or_else(|e| e.into_inner());
    let found = pages
        .iter_mut()
        .find(|(start, used)| start.abs_diff(near) < REACH && used + size <= PAGE);
    let (start, used) = match found {
        Some(page) => page,
        None => {
            let page = alloc_near(near).map_err(Error::Jvm)?;
            pages.push((page, 0));
            pages.last_mut().unwrap()
        }
    };
    let at = *start + *used;
    *used += size;
    unsafe {
        std::ptr::copy_nonoverlapping(code.as_ptr(), at as *mut u8, code.len());
        FlushInstructionCache(GetCurrentProcess(), at as *const c_void, code.len());
    }
    Ok(at)
}

#[link(name = "kernel32")]
extern "system" {
    fn VirtualAlloc(address: *mut c_void, size: usize, kind: u32, protect: u32) -> *mut c_void;
    pub(crate) fn FlushInstructionCache(process: *mut c_void, address: *const c_void, size: usize) -> i32;
    pub(crate) fn GetCurrentProcess() -> *mut c_void;
}
const MEM_COMMIT_RESERVE: u32 = 0x3000;
const PAGE_EXECUTE_READWRITE: u32 = 0x40;
pub(crate) const PAGE: usize = 0x1000;
const GRANULARITY: usize = 0x10000;
const REACH: usize = 0x4000_0000;

static PAGES: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());

pub(crate) fn alloc_near(near: usize) -> std::result::Result<usize, String> {
    alloc_near_size(near, PAGE)
}

pub(crate) fn alloc_near_size(near: usize, size: usize) -> std::result::Result<usize, String> {
    let size = size.max(PAGE).next_multiple_of(PAGE);
    let origin = near & !(GRANULARITY - 1);
    for offset in (GRANULARITY..REACH).step_by(GRANULARITY) {
        for candidate in [origin.checked_add(offset), origin.checked_sub(offset)].into_iter().flatten() {
            let page = unsafe {
                VirtualAlloc(candidate as *mut c_void, size, MEM_COMMIT_RESERVE, PAGE_EXECUTE_READWRITE)
            };
            if !page.is_null() {
                return Ok(page as usize);
            }
        }
    }
    Err("no free memory near Slinky".into())
}
