use crate::automap::Map;
use crate::error::{Error, Result};
use std::ffi::CString;

use super::layout::{
    NOTIFY_BUF_LEN, NOTIFY_RING, NOTIFY_TYPE_INFO, SLOT_STRIDE, SLOT_TEXT_OFFSET,
    SLOT_TIMER_OFFSET,
};
use super::mem::{read_u64, write_f32, write_i32, write_memory, write_u64};

pub fn notify_info(message: &str) -> Result<()> {
    notify_with_map(crate::automap::get()?, message)
}

pub fn notify_with_map(map: &Map, message: &str) -> Result<()> {
    push_manual(map, NOTIFY_TYPE_INFO, message)
}

fn push_manual(map: &Map, slot_type: i32, text: &str) -> Result<()> {
    map.require(crate::automap::notifications::RING_REQUIRED)?;
    let text = slot_text(text)?;
    let counter_a_at = map.address("queue_counter_a")?;
    let counter_b_at = map.address("queue_counter_b")?;
    let counter_a = read_u64(counter_a_at).ok_or_else(|| {
        Error::Jvm(format!("could not read queue counter A @ {counter_a_at:#x}"))
    })?;
    let counter_b = read_u64(counter_b_at).ok_or_else(|| {
        Error::Jvm(format!("could not read queue counter B @ {counter_b_at:#x}"))
    })?;

    let index = (counter_a + counter_b) % NOTIFY_RING;
    let meta_slot = map.address("slot_array")? + (index as usize) * SLOT_STRIDE;
    let meta_text = meta_slot + SLOT_TEXT_OFFSET;

    if !write_i32(meta_slot, slot_type) {
        return Err(Error::Jvm(format!(
            "failed to write slot type @ {:#x}",
            meta_slot
        )));
    }
    if !write_f32(meta_slot + SLOT_TIMER_OFFSET, 0.0) {
        return Err(Error::Jvm(format!(
            "failed to write slot timer @ {:#x}",
            meta_slot + SLOT_TIMER_OFFSET
        )));
    }
    if !write_memory(meta_text, &text) {
        return Err(Error::Jvm(format!("failed to write text @ {meta_text:#x}")));
    }

    if counter_b == NOTIFY_RING {
        if !write_u64(counter_a_at, counter_a + 1) {
            return Err(Error::Jvm(format!(
                "failed to bump counter A @ {counter_a_at:#x}"
            )));
        }
    } else if !write_u64(counter_b_at, counter_b + 1) {
        return Err(Error::Jvm(format!(
            "failed to bump counter B @ {counter_b_at:#x}"
        )));
    }
    Ok(())
}

fn slot_text(text: &str) -> Result<Vec<u8>> {
    let mut end = text.len().min(NOTIFY_BUF_LEN - 1);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let text = CString::new(&text[..end]).map_err(|e| Error::Jvm(e.to_string()))?;
    let bytes = text.as_bytes_with_nul();
    let mut padded = vec![0u8; NOTIFY_BUF_LEN];
    padded[..bytes.len()].copy_from_slice(bytes);
    Ok(padded)
}
