use super::mem::read_u64;
use crate::{automap, Error, Result};
use std::ffi::c_void;

pub trait Access {
    fn current_item(&self) -> Result<i32>;
    fn select(&self, slot: i32) -> Result<()>;
}

type Claim = unsafe extern "system" fn(*const c_void) -> u8;
type Release = unsafe extern "system" fn(*const c_void);

#[derive(Clone, Copy)]
pub struct Hotbar {
    claim: Claim,
    release: Release,
    owner: usize,
}

impl Hotbar {
    pub fn resolve(map: &automap::Map) -> Result<Self> {
        map.require(automap::hotbar::OWNERSHIP_REQUIRED)?;
        unsafe {
            Ok(Self {
                claim: std::mem::transmute::<usize, Claim>(map.address("item_switch_claim")?),
                release: std::mem::transmute::<usize, Release>(map.address("item_switch_release")?),
                owner: map.address("item_switch_owner")?,
            })
        }
    }
    pub fn owner(&self) -> Result<usize> {
        read_u64(self.owner)
            .map(|value| value as usize)
            .ok_or_else(|| Error::Jvm("native item-switch owner is unreadable".into()))
    }
    pub fn session(self) -> Session {
        Session {
            native: self,
            token: Box::new(0),
            saved: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    Inactive,
    Released,
    Restored,
    ManualChange,
    LostOwnership,
}

#[derive(Clone, Copy)]
struct Saved {
    original: i32,
    selected: i32,
}

pub struct Session {
    native: Hotbar,
    token: Box<u8>,
    saved: Option<Saved>,
}

impl Session {
    fn key(&self) -> *const c_void {
        (&*self.token as *const u8).cast()
    }
    pub fn active(&self) -> bool {
        self.saved.is_some()
    }
    pub fn begin(&mut self, inventory: &impl Access) -> Result<bool> {
        if self.active() {
            return Ok(self.native.owner()? == self.key() as usize);
        }
        let original = inventory.current_item()?;
        valid_slot(original)?;
        if unsafe { (self.native.claim)(self.key()) } == 0 {
            return Ok(false);
        }
        if self.native.owner()? != self.key() as usize {
            unsafe { (self.native.release)(self.key()) };
            return Err(Error::Jvm(
                "native item-switch claim returned success without ownership".into(),
            ));
        }
        self.saved = Some(Saved {
            original,
            selected: original,
        });
        Ok(true)
    }
    pub fn select(&mut self, inventory: &impl Access, slot: i32) -> Result<()> {
        valid_slot(slot)?;
        let saved = self
            .saved
            .ok_or_else(|| Error::Jvm("hotbar session has not acquired ownership".into()))?;
        if self.native.owner()? != self.key() as usize {
            return Err(Error::Jvm("hotbar session lost native ownership".into()));
        }
        if inventory.current_item()? != saved.selected {
            return Err(Error::Jvm(
                "manual hotbar change interrupted this session".into(),
            ));
        }
        if saved.selected != slot {
            inventory.select(slot)?;
        }
        self.saved = Some(Saved {
            selected: slot,
            ..saved
        });
        Ok(())
    }
    pub fn finish(&mut self, inventory: &impl Access, restore: bool) -> Result<Finish> {
        let Some(saved) = self.saved.take() else {
            return Ok(Finish::Inactive);
        };
        if self.native.owner()? != self.key() as usize {
            return Ok(Finish::LostOwnership);
        }
        let result = (|| {
            let current = inventory.current_item()?;
            if current != saved.selected {
                return Ok(Finish::ManualChange);
            }
            if restore && current != saved.original {
                inventory.select(saved.original)?;
                Ok(Finish::Restored)
            } else {
                Ok(Finish::Released)
            }
        })();
        unsafe { (self.native.release)(self.key()) };
        result
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { (self.native.release)(self.key()) };
    }
}

fn valid_slot(slot: i32) -> Result<()> {
    if (0..9).contains(&slot) {
        Ok(())
    } else {
        Err(Error::Jvm(format!(
            "invalid hotbar slot {slot}; expected 0..8"
        )))
    }
}
