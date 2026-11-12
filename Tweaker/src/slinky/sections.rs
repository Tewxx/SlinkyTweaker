use super::mem::read_memory;
use super::settings::{self as native_settings, Setting};
use super::thunk::{self, Arg, FlushInstructionCache, GetCurrentProcess, PAGE};
use crate::automap::Map;
use crate::{Error, Result};
use iced_x86::{
    BlockEncoder, BlockEncoderOptions, Decoder, DecoderOptions, FlowControl, Instruction,
    InstructionBlock,
};
use std::ffi::{c_char, c_void, CStr};

type HeaderFn = unsafe extern "system" fn(*mut c_void, *mut c_void, *const c_char);

struct Section {
    title: &'static CStr,
    settings: Vec<Setting>,
    header: HeaderFn,
}

pub(crate) fn insert_before(
    map: &Map,
    module: &str,
    gui: usize,
    before: &str,
    title: &'static CStr,
    settings: &[Setting],
) -> Result<usize> {
    let fail = |why: String| Error::Jvm(format!("{module}: \"{before}\" section: {why}"));
    let header = map.address("gui_section")?;
    let calls: Vec<usize> = map
        .sections
        .get(module)
        .into_iter()
        .flatten()
        .filter(|(text, _)| text == before)
        .map(|&(_, call)| map.base + call)
        .collect();
    let [call] = calls[..] else {
        return Err(fail(format!("expected one header, found {}", calls.len())));
    };
    let mut body = callback_body(gui).map_err(fail)?;
    let site = body
        .iter_mut()
        .find(|i| i.ip() as usize == call)
        .filter(|i| i.flow_control() == FlowControl::Call && i.near_branch_target() as usize == header)
        .ok_or_else(|| fail("header call not in the callback".into()))?;

    let section: &'static Section = Box::leak(Box::new(Section {
        title,
        settings: settings.to_vec(),
        header: unsafe { std::mem::transmute::<usize, HeaderFn>(header) },
    }));
    let stub = thunk::bind(gui, Arg::Fourth, section as *const Section as usize, draw_before as *const () as usize)?;
    site.set_near_branch64(stub as u64);

    let copy = thunk::alloc_near(gui).map_err(fail)?;
    let encoded = BlockEncoder::encode(64, InstructionBlock::new(&body, copy as u64), BlockEncoderOptions::NONE)
        .map_err(|e| fail(format!("relocating the callback: {e}")))?;
    let code = encoded.code_buffer;
    if code.len() > PAGE {
        return Err(fail(format!("callback too large ({} bytes)", code.len())));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(code.as_ptr(), copy as *mut u8, code.len());
        FlushInstructionCache(GetCurrentProcess(), copy as *const c_void, code.len());
    }
    Ok(copy)
}

pub(crate) fn callback_body(entry: usize) -> std::result::Result<Vec<Instruction>, String> {
    const MAX: usize = 0x1000;
    let bytes = read_memory(entry, MAX).ok_or("callback unreadable")?;
    let mut decoder = Decoder::with_ip(64, &bytes, entry as u64, DecoderOptions::NONE);
    let inside = |target: u64| (entry as u64..(entry + MAX) as u64).contains(&target);
    let mut body = Vec::new();
    let mut reach = entry as u64;
    while decoder.can_decode() {
        let i = decoder.decode();
        if i.is_invalid() {
            return Err(format!("invalid instruction at {:#x}", i.ip()));
        }
        body.push(i);
        match i.flow_control() {
            FlowControl::IndirectBranch => return Err("callback uses a jump table".into()),
            FlowControl::ConditionalBranch | FlowControl::UnconditionalBranch
                if inside(i.near_branch_target()) =>
            {
                reach = reach.max(i.near_branch_target());
            }
            _ => {}
        }
        let exits = matches!(
            i.flow_control(),
            FlowControl::Return | FlowControl::UnconditionalBranch | FlowControl::Exception
        );
        if exits && i.next_ip() > reach {
            return Ok(body);
        }
    }
    Err("callback end not found".into())
}

unsafe extern "system" fn draw_before(
    gui: *mut c_void,
    layout: *mut c_void,
    text: *const c_char,
    section: *const Section,
) {
    let s = unsafe { &*section };
    unsafe {
        (s.header)(gui, layout, s.title.as_ptr());
        native_settings::gui(&s.settings, gui, layout);
        (s.header)(gui, layout, text);
    }
}
