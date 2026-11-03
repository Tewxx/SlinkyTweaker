mod catalog;
use super::mem::{read_cstr, read_memory, read_u64, write_u64};
use crate::error::{Error, Result};
pub(crate) use catalog::{Catalog, ModuleSpec, MODULE_ENABLED};
use std::ffi::{c_void, CString};
pub(crate) fn module_title(module: usize) -> Option<String> {
    let title = read_u64(module)? as usize;
    if title == 0 {
        return None;
    }
    read_cstr(title, 64)
}

pub(crate) fn sort_modules(modules: &mut [usize]) {
    modules
        .sort_by_cached_key(|&entry| module_title(entry).unwrap_or_default().to_ascii_lowercase());
}

pub(crate) fn read_usize(address: usize, what: &str) -> Result<usize> {
    read_u64(address)
        .map(|value| value as usize)
        .ok_or_else(|| Error::Jvm(format!("failed to read {what} @ {address:#x}")))
}

pub(crate) fn read_pointer_array(address: usize, count: usize, what: &str) -> Result<Vec<usize>> {
    if address == 0 && count != 0 {
        return Err(Error::Jvm(format!("{what} pointer is null")));
    }
    let bytes = read_memory(address, count * 8)
        .ok_or_else(|| Error::Jvm(format!("failed to read {what} @ {address:#x}")))?;
    Ok(bytes
        .chunks_exact(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()) as usize)
        .collect())
}

pub(crate) fn read_null_terminated_pointer_array(
    address: usize,
    max_count: usize,
    what: &str,
) -> Result<Vec<usize>> {
    if address == 0 {
        return Err(Error::Jvm(format!("{what} pointer is null")));
    }
    let mut values = Vec::new();
    for index in 0..max_count {
        let value = read_usize(address + index * 8, what)?;
        if value == 0 {
            return Ok(values);
        }
        values.push(value);
    }
    Err(Error::Jvm(format!(
        "{what} has no terminator within {max_count} entries"
    )))
}

pub(crate) fn copy_pointer_prefix(
    source: usize,
    count: usize,
    destination: &mut [usize],
    what: &str,
) -> Result<()> {
    if count > destination.len() {
        return Err(Error::Jvm(format!("{what} count is invalid: {count}")));
    }
    if count == 0 {
        return Ok(());
    }
    let existing = read_pointer_array(source, count, what)?;
    destination[..count].copy_from_slice(&existing);
    Ok(())
}

pub(crate) fn clone_aligned(address: usize, size: usize, what: &str) -> Result<Vec<u64>> {
    let bytes = read_memory(address, size)
        .ok_or_else(|| Error::Jvm(format!("failed to read {what} @ {address:#x}")))?;
    let mut words = vec![0u64; size.div_ceil(8)];
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), words.as_mut_ptr().cast(), size);
    }
    Ok(words)
}

pub(crate) fn bytes_mut(words: &mut [u64]) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut(words.as_mut_ptr().cast(), words.len() * 8) }
}

pub(crate) fn set_usize(words: &mut [u64], offset: usize, value: usize) {
    bytes_mut(words)[offset..offset + 8].copy_from_slice(&(value as u64).to_le_bytes());
}

pub(crate) fn set_u32(words: &mut [u64], offset: usize, value: u32) {
    bytes_mut(words)[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn set_u8(words: &mut [u64], offset: usize, value: u8) {
    bytes_mut(words)[offset] = value;
}

pub(crate) fn set_f32(words: &mut [u64], offset: usize, value: f32) {
    bytes_mut(words)[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn zero_range(words: &mut [u64], begin: usize, end: usize) {
    bytes_mut(words)[begin..end].fill(0);
}

pub(crate) fn leak_words(words: Vec<u64>) -> usize {
    Box::into_raw(words.into_boxed_slice()) as *mut u64 as usize
}

pub(crate) fn leak_pointer_array(values: Vec<usize>) -> usize {
    Box::into_raw(values.into_boxed_slice()) as *mut usize as usize
}

pub(crate) fn leak_cstring(value: &str) -> Result<usize> {
    CString::new(value)
        .map(|value| value.into_raw() as usize)
        .map_err(|error| Error::Jvm(error.to_string()))
}

pub(crate) fn write_ptr(address: usize, value: usize, what: &str) -> Result<()> {
    if write_u64(address, value as u64) {
        Ok(())
    } else {
        Err(Error::Jvm(format!("failed to write {what} @ {address:#x}")))
    }
}

pub(crate) fn prepare_listeners(
    manager: usize,
    callbacks: &[(usize, usize, bool)],
) -> Result<Vec<(usize, usize)>> {
    if callbacks.iter().any(|&(slot, _, _)| slot >= 27) {
        return Err(Error::Jvm("invalid custom listener slot".into()));
    }
    let mut slots: Vec<usize> = callbacks.iter().map(|&(slot, _, _)| slot).collect();
    slots.sort_unstable();
    slots.dedup();
    let mut updates = Vec::new();
    for slot in slots {
        let candidates_at = manager + 0x60 + slot * 0x10;
        let count = read_usize(candidates_at + 8, "listener candidate count")?;
        if count > 256 {
            return Err(Error::Jvm("unsupported listener candidate count".into()));
        }
        let mut candidates = read_pointer_array(
            read_usize(candidates_at, "listener candidates")?,
            count,
            "listener candidates",
        )?;
        let active_at = manager + 0x210 + slot * 8;
        let mut active = read_null_terminated_pointer_array(
            read_usize(active_at, "active listeners")?,
            256,
            "active listeners",
        )?;
        let mut front = 0;
        for &(_, callback, first) in callbacks.iter().filter(|c| c.0 == slot) {
            let mut owner = vec![0u64; 0x210 / 8];
            set_u8(&mut owner, 0x14, 1);
            set_usize(&mut owner, 0x48 + slot * 0x10, callback);
            let owner = leak_words(owner);
            if first {
                candidates.insert(front, owner);
                active.insert(front, callback);
                front += 1;
            } else {
                candidates.push(owner);
                active.push(callback);
            }
        }
        let added = candidates.len() - count;
        active.resize(listener_capacity(candidates.len(), active.len()), 0);
        updates.push((active_at, leak_pointer_array(active)));
        updates.push((candidates_at, leak_pointer_array(candidates)));
        updates.push((candidates_at + 8, count + added));
    }
    Ok(updates)
}

pub(crate) fn publish_listeners(base: usize, callbacks: &[(usize, usize, bool)]) -> Result<()> {
    crate::automap::require(&["module_manager"])?;
    let writes = prepare_listeners(base + crate::automap::rva("module_manager"), callbacks)?;
    let _threads = SuspendedThreads::suspend_others()?;
    for &(at, value) in &writes {
        write_ptr(at, value, "listener publication")?;
    }
    Ok(())
}

pub(crate) fn catalog_modules(base: usize) -> Result<Vec<usize>> {
    crate::automap::require(&["module_manager"])?;
    let manager = base + crate::automap::rva("module_manager");
    let count = read_usize(manager + 8, "module count")?;
    if count > 1024 {
        return Err(Error::Jvm(format!("implausible module count {count}")));
    }
    read_pointer_array(read_usize(manager, "module array")?, count, "modules")
}

pub(crate) fn active_listeners(base: usize, slot: usize) -> Result<usize> {
    crate::automap::require(&["module_manager"])?;
    read_usize(
        base + crate::automap::rva("module_manager") + 0x210 + slot * 8,
        "active listeners",
    )
}

fn listener_capacity(candidates: usize, active: usize) -> usize {
    candidates.max(active) + 1
}

const TH32CS_SNAPTHREAD: u32 = 0x00000004;
const THREAD_SUSPEND_RESUME: u32 = 0x0002;
const INVALID_HANDLE_VALUE: isize = -1;

#[repr(C)]
struct ThreadEntry32 {
    size: u32,
    usage: u32,
    thread_id: u32,
    owner_process_id: u32,
    base_priority: i32,
    priority_delta: i32,
    flags: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
    fn Thread32First(snapshot: *mut c_void, entry: *mut ThreadEntry32) -> i32;
    fn Thread32Next(snapshot: *mut c_void, entry: *mut ThreadEntry32) -> i32;
    fn OpenThread(access: u32, inherit: i32, thread_id: u32) -> *mut c_void;
    fn SuspendThread(thread: *mut c_void) -> u32;
    fn ResumeThread(thread: *mut c_void) -> u32;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn GetCurrentProcessId() -> u32;
    fn GetCurrentThreadId() -> u32;
}

pub(crate) struct SuspendedThreads {
    handles: Vec<*mut c_void>,
}

impl SuspendedThreads {
    pub(crate) fn suspend_others() -> Result<Self> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot as isize == INVALID_HANDLE_VALUE {
            return Err(Error::Jvm("CreateToolhelp32Snapshot failed".into()));
        }

        let process_id = unsafe { GetCurrentProcessId() };
        let current_thread = unsafe { GetCurrentThreadId() };
        let mut entry = ThreadEntry32 {
            size: std::mem::size_of::<ThreadEntry32>() as u32,
            usage: 0,
            thread_id: 0,
            owner_process_id: 0,
            base_priority: 0,
            priority_delta: 0,
            flags: 0,
        };
        let mut handles = Vec::new();
        let mut has_entry = unsafe { Thread32First(snapshot, &mut entry) } != 0;

        while has_entry {
            if entry.owner_process_id == process_id && entry.thread_id != current_thread {
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.thread_id) };
                if !thread.is_null() {
                    let result = unsafe { SuspendThread(thread) };
                    if result != u32::MAX {
                        handles.push(thread);
                    } else {
                        unsafe { CloseHandle(thread) };
                    }
                }
            }
            has_entry = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
        }
        unsafe { CloseHandle(snapshot) };
        Ok(Self { handles })
    }
}

impl Drop for SuspendedThreads {
    fn drop(&mut self) {
        for &thread in self.handles.iter().rev() {
            unsafe {
                ResumeThread(thread);
                CloseHandle(thread);
            }
        }
    }
}
