use crate::slinky::mem::read_memory;
use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

const PARAMS_CALLBACKS: std::ops::Range<usize> = 0x10..0x70;
const COMMANDS: usize = 0x70;

static VALIDATED: AtomicUsize = AtomicUsize::new(0);

fn inside(image: (usize, usize), address: usize) -> bool {
    address >= image.0 && address < image.0 + image.1
}

fn valid_context(image: (usize, usize), ctx: usize) -> bool {
    if ctx == 0 {
        return false;
    }
    if VALIDATED.load(Ordering::Acquire) == ctx {
        return true;
    }
    let Some(bytes) = read_memory(ctx, COMMANDS + 0x10) else {
        return false;
    };
    let word = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize;
    let anti_alias = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let capacity = i32::from_le_bytes(bytes[COMMANDS + 8..COMMANDS + 12].try_into().unwrap());
    let count = i32::from_le_bytes(bytes[COMMANDS + 12..COMMANDS + 16].try_into().unwrap());
    let valid = anti_alias <= 1
        && PARAMS_CALLBACKS
            .step_by(8)
            .all(|at| inside(image, word(at)))
        && word(COMMANDS) != 0
        && (0..=capacity).contains(&count);
    if valid {
        VALIDATED.store(ctx, Ordering::Release);
    }
    valid
}

pub(crate) fn resolve(candidates: [usize; 2]) -> Option<*mut c_void> {
    let map = crate::automap::get().ok()?;
    let image = (map.base, map.image_size);
    candidates
        .into_iter()
        .find(|&ctx| valid_context(image, ctx))
        .map(|ctx| ctx as *mut c_void)
}
