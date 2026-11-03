use crate::error::{Error, Result};
use std::ffi::CString;

use super::layout::{
    NOTIFY_BUF_LEN, NOTIFY_RING, NOTIFY_TYPE_INFO, SLOT_STRIDE, SLOT_TEXT_OFFSET,
    SLOT_TIMER_OFFSET,
};
use super::mem::{read_u64, write_f32, write_i32, write_memory, write_u64};
use super::SlinkyModule;
use crate::automap::rva;

pub fn notify_info(module: &SlinkyModule, message: &str) -> Result<()> {
    push_manual(module, NOTIFY_TYPE_INFO, message)
}

fn push_manual(module: &SlinkyModule, slot_type: i32, text: &str) -> Result<()> {
    crate::automap::require(crate::automap::notifications::RING_REQUIRED)?;
    let base = module.base;
    let counter_a = read_u64(base + rva("queue_counter_a")).ok_or_else(|| {
        Error::Jvm(format!(
            "could not read queue counter A @ {:#x}",
            base + rva("queue_counter_a")
        ))
    })?;
    let counter_b = read_u64(base + rva("queue_counter_b")).ok_or_else(|| {
        Error::Jvm(format!(
            "could not read queue counter B @ {:#x}",
            base + rva("queue_counter_b")
        ))
    })?;

    let index = (counter_a + counter_b) % NOTIFY_RING;
    let meta_slot = base + rva("slot_array") + (index as usize) * SLOT_STRIDE;
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

    if counter_b == NOTIFY_RING {
        if !write_u64(base + rva("queue_counter_a"), counter_a + 1) {
            return Err(Error::Jvm(format!(
                "failed to bump counter A @ {:#x}",
                base + rva("queue_counter_a")
            )));
        }
    } else if !write_u64(base + rva("queue_counter_b"), counter_b + 1) {
        return Err(Error::Jvm(format!(
            "failed to bump counter B @ {:#x}",
            base + rva("queue_counter_b")
        )));
    }

    write_slot_text(meta_text, text)?;
    Ok(())
}

fn write_slot_text(dest: usize, text: &str) -> Result<()> {
    let text = CString::new(text).map_err(|e| Error::Jvm(e.to_string()))?;
    let bytes = text.as_bytes_with_nul();
    if bytes.len() > NOTIFY_BUF_LEN {
        return Err(Error::Jvm(format!(
            "notification too long ({} > {})",
            bytes.len(),
            NOTIFY_BUF_LEN
        )));
    }

    let mut padded = vec![0u8; NOTIFY_BUF_LEN];
    padded[..bytes.len()].copy_from_slice(bytes);
    if !write_memory(dest, &padded) {
        return Err(Error::Jvm(format!("failed to write text @ {:#x}", dest)));
    }
    Ok(())
}
