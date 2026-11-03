use std::ffi::c_void;

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn ReadProcessMemory(
        process: *mut c_void,
        base_address: *const c_void,
        buffer: *mut c_void,
        size: usize,
        bytes_read: *mut usize,
    ) -> i32;
    fn WriteProcessMemory(
        process: *mut c_void,
        base_address: *mut c_void,
        buffer: *const c_void,
        size: usize,
        bytes_written: *mut usize,
    ) -> i32;
}

pub fn read_memory(addr: usize, len: usize) -> Option<Vec<u8>> {
    if len == 0 {
        return Some(Vec::new());
    }
    let mut out = vec![0u8; len];
    let mut read = 0usize;
    let ok = unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            addr as *const c_void,
            out.as_mut_ptr() as *mut c_void,
            len,
            &mut read,
        )
    };
    if ok == 0 || read != len {
        return None;
    }
    Some(out)
}

pub fn write_memory(addr: usize, data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    let mut written = 0usize;
    let ok = unsafe {
        WriteProcessMemory(
            GetCurrentProcess(),
            addr as *mut c_void,
            data.as_ptr() as *const c_void,
            data.len(),
            &mut written,
        )
    };
    ok != 0 && written == data.len()
}

pub fn read_u64(addr: usize) -> Option<u64> {
    read_memory(addr, 8).map(|b| u64::from_le_bytes(b[..8].try_into().unwrap()))
}

pub fn read_u32(addr: usize) -> Option<u32> {
    read_memory(addr, 4).map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))
}

pub fn read_flag(addr: usize) -> bool {
    read_memory(addr, 1).as_deref() == Some(&[1])
}

pub fn read_f32(addr: usize) -> Option<f32> {
    read_memory(addr, 4).map(|b| f32::from_le_bytes(b[..4].try_into().unwrap()))
}

pub fn write_u64(addr: usize, value: u64) -> bool {
    write_memory(addr, &value.to_le_bytes())
}

pub fn write_i32(addr: usize, value: i32) -> bool {
    write_memory(addr, &value.to_le_bytes())
}

pub fn write_f32(addr: usize, value: f32) -> bool {
    write_memory(addr, &value.to_le_bytes())
}

pub fn read_cstr(addr: usize, max_len: usize) -> Option<String> {
    let bytes = read_memory(addr, max_len)?;
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    if end == 0 {
        return None;
    }
    String::from_utf8(bytes[..end].to_vec()).ok()
}
