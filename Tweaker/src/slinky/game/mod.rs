mod combat;
mod entity;
mod input;

mod world;

use crate::automap::{jni::EventLayout, Map};
use crate::slinky::mem::{read_memory, read_u64};
use crate::{Error, Result};
use jni_simple::JNIEnv;
use std::{collections::BTreeMap, ffi::c_void, marker::PhantomData, rc::Rc};

pub use input::Binding;

type Obj = *mut c_void;
type Head = *mut c_void;

macro_rules! handle {
    ($($name:ident),*) => {$(
        #[derive(Clone, Copy, Debug)]
        pub struct $name<'frame> { raw: Obj, scope: PhantomData<(&'frame (), Rc<()>)> }
        impl<'frame> $name<'frame> {
            fn new(raw: Obj) -> Option<Self> { (!raw.is_null()).then_some(Self { raw, scope: PhantomData }) }
        }
    )*};
}
handle!(Key);

#[derive(Clone, Copy, Debug)]
pub struct Entity<'frame> {
    raw: Obj,
    player: bool,
    scope: PhantomData<(&'frame (), Rc<()>)>,
}
impl<'frame> Entity<'frame> {
    fn new(raw: Obj) -> Option<Self> {
        (!raw.is_null()).then_some(Self {
            raw,
            player: false,
            scope: PhantomData,
        })
    }
    fn as_player(raw: Obj) -> Option<Self> {
        Self::new(raw).map(|mut e| {
            e.player = true;
            e
        })
    }
    fn require_player(self) -> Result<()> {
        if self.player {
            Ok(())
        } else {
            Err(Error::Jvm(
                "this method requires a player handle from player()/players()".into(),
            ))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Angles {
    pub yaw: f32,
    pub pitch: f32,
}
impl Angles {
    pub fn from_packed(value: u64) -> Result<Self> {
        let result = Self {
            yaw: f32::from_bits(value as u32),
            pitch: f32::from_bits((value >> 32) as u32),
        };
        if result.yaw.is_finite() && result.pitch.is_finite() {
            Ok(result)
        } else {
            Err(Error::Jvm("native angles were not finite".into()))
        }
    }
}

pub struct Natives {
    layout: EventLayout,
    addresses: BTreeMap<String, usize>,
}

impl Natives {
    pub fn resolve(map: &Map) -> Result<Self> {
        let layout = map
            .event_layout
            .ok_or_else(|| Error::Jvm("native game-event layout unavailable".into()))?;
        let addresses = map
            .anchors
            .iter()
            .filter(|(_, a)| a.rva < map.image_size)
            .map(|(name, a)| (name.clone(), map.base + a.rva))
            .collect();
        Ok(Self { layout, addresses })
    }
    fn address(&self, name: &str) -> Result<usize> {
        self.addresses
            .get(name)
            .copied()
            .ok_or_else(|| Error::Jvm(format!("native helper {name} unavailable")))
    }

    pub unsafe fn frame<R>(
        &self,
        context: *mut c_void,
        f: impl for<'frame> FnOnce(&Game<'frame>) -> Result<R>,
    ) -> Result<Option<R>> {
        if context.is_null() {
            return Ok(None);
        }
        let Some(event) =
            read_u64(context as usize + self.layout.context_event).filter(|&p| p != 0)
        else {
            return Ok(None);
        };
        let event = event as usize;
        if read_memory(event + self.layout.phase, 1).as_deref() != Some(&[1]) {
            return Ok(None);
        }
        let field = |offset| {
            read_u64(event + offset)
                .filter(|&p| p != 0)
                .map(|p| p as Obj)
        };
        let (Some(head), Some(minecraft), Some(player)) = (
            field(self.layout.jni_head),
            field(self.layout.minecraft),
            field(self.layout.player),
        ) else {
            return Ok(None);
        };
        let env_raw = read_u64(head as usize + 8)
            .filter(|&p| p != 0)
            .ok_or_else(|| Error::Jvm("native JNI environment unavailable".into()))?
            as usize;
        let table = read_u64(env_raw)
            .filter(|&p| p != 0)
            .ok_or_else(|| Error::Jvm("native JNI table unavailable".into()))?
            as usize;
        for offset in [0x88, 0x98, 0xA0, 0x720] {
            if read_u64(table + offset).is_none_or(|p| p == 0) {
                return Err(Error::Jvm(
                    "native JNI local-frame functions unavailable".into(),
                ));
            }
        }
        let env = JNIEnv::from(env_raw as Obj);
        unsafe {
            if env.ExceptionCheck() {
                return Err(Error::Jvm(
                    "JNI exception already pending before native frame; left untouched"
                        .into(),
                ));
            }
            if env.PushLocalFrame(256) != 0 {
                if env.ExceptionCheck() {
                    env.ExceptionClear();
                }
                return Err(Error::Jvm(
                    "cannot allocate native local frame".into(),
                ));
            }
        }
        let _frame = LocalFrame(env);
        let mut game = Game {
            natives: self,
            env,
            event: event as Obj,
            head,
            minecraft,
            player,
            world: std::ptr::null_mut(),
            scope: PhantomData,
        };
        game.world = native!(
            &game,
            "native_world",
            unsafe extern "system" fn(Head, Obj) -> Obj,
            head,
            minecraft
        )?;
        if game.world.is_null() {
            return Ok(None);
        }
        let result = f(&game)?;
        game.check("native callback")?;
        Ok(Some(result))
    }
}

struct LocalFrame(JNIEnv);
impl Drop for LocalFrame {
    fn drop(&mut self) {
        unsafe {
            if self.0.ExceptionCheck() {
                self.0.ExceptionClear();
            }
            self.0.PopLocalFrame(std::ptr::null_mut());
        }
    }
}

pub struct Game<'frame> {
    natives: &'frame Natives,
    env: JNIEnv,
    event: Obj,
    head: Head,
    minecraft: Obj,
    player: Obj,
    world: Obj,
    scope: PhantomData<Rc<()>>,
}

macro_rules! native {
    ($game:expr, $name:expr, $ty:ty $(, $arg:expr)* $(,)?) => {{
        let game = $game;
        let name = $name;
        let function: $ty = unsafe { std::mem::transmute(game.natives.address(name)?) };
        let result = unsafe { function($($arg),*) };
        game.check(name)?;
        Ok::<_, Error>(result)
    }};
}
use native;

impl<'frame> Game<'frame> {
    fn check(&self, operation: &str) -> Result<()> {
        if unsafe { self.env.ExceptionCheck() } {
            unsafe {
                self.env.ExceptionClear();
            }
            Err(Error::Jvm(format!(
                "Java exception in native service {operation}"
            )))
        } else {
            Ok(())
        }
    }
    fn object(&self, name: &str, object: Obj) -> Result<Obj> {
        native!(
            self,
            name,
            unsafe extern "system" fn(Head, Obj) -> Obj,
            self.head,
            object
        )
    }
    fn integer(&self, name: &str, object: Obj) -> Result<i32> {
        native!(
            self,
            name,
            unsafe extern "system" fn(Head, Obj) -> i32,
            self.head,
            object
        )
    }
    fn boolean(&self, name: &str, object: Obj) -> Result<bool> {
        let value = native!(
            self,
            name,
            unsafe extern "system" fn(Head, Obj) -> u8,
            self.head,
            object
        )?;
        match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::Jvm(format!(
                "invalid native boolean from {name}: {value}"
            ))),
        }
    }
    fn vector(&self, name: &str, object: Obj) -> Result<[f64; 3]> {
        let mut out = [f64::NAN; 3];
        native!(
            self,
            name,
            unsafe extern "system" fn(*mut [f64; 3], Head, Obj) -> *mut [f64; 3],
            &mut out,
            self.head,
            object
        )?;
        if out.iter().all(|v| v.is_finite()) {
            Ok(out)
        } else {
            Err(Error::Jvm(format!("invalid native vector from {name}")))
        }
    }
    pub fn angles(&self) -> Result<Angles> {
        Angles::from_packed(native!(
            self,
            "jni_player_angles",
            unsafe extern "system" fn(Head, Obj) -> u64,
            self.head,
            self.player
        )?)
    }
}
