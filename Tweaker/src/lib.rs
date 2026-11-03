#![allow(non_snake_case)]

pub(crate) mod automap;
pub(crate) mod error;
pub(crate) mod gui;
pub(crate) mod jvm;
pub(crate) mod minecraft;
pub(crate) mod module;
pub(crate) mod platform;
pub(crate) mod slinky;
mod startup;

pub(crate) use error::{Error, Result};

use std::thread;

const DLL_PROCESS_ATTACH: u32 = 1;

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
extern "system" fn DllMain(_module: usize, reason: u32, _reserved: usize) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        thread::spawn(startup::start);
    }
    1
}
