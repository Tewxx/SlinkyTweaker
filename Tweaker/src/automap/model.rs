use super::{events::Event, image::Image, jni, scan};
use crate::{Error, Result};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Anchor {
    pub rva: usize,
}
#[derive(Clone, Debug)]
pub struct Hook {
    pub slot: usize,
    pub priority: i32,
    pub callback: usize,
}
#[derive(Clone, Debug)]
pub struct Setting {
    pub key: String,
    pub kind: String,
    pub descriptor: usize,
    pub value_offset: Option<usize>,
    pub gui_helper: Option<usize>,
}
#[derive(Clone, Debug)]
pub struct Module {
    pub name: String,
    pub descriptor: usize,
    pub gui_callback: Option<usize>,
    pub sync_callback: Option<usize>,
    pub hooks: Vec<Hook>,
    pub settings: BTreeMap<String, Setting>,
}
#[derive(Default, Debug)]
pub struct Map {
    pub base: usize,
    pub image_size: usize,
    pub anchors: BTreeMap<String, Anchor>,
    pub modules: BTreeMap<String, Module>,
    pub sections: BTreeMap<String, Vec<(String, usize)>>,
    pub events: BTreeMap<Event, usize>,
    pub event_layout: Option<jni::EventLayout>,
    pub text_box: Option<super::text_box::TextBox>,
    pub native_silent_mode: Option<u32>,
}

impl Map {
    pub fn address(&self, name: &str) -> Result<usize> {
        self.anchors
            .get(name)
            .map(|anchor| self.base + anchor.rva)
            .ok_or_else(|| Error::Jvm(format!("automap unresolved anchor {name}")))
    }
    pub fn insert(&mut self, name: &str, rva: usize) {
        if let Some(previous) = self.anchors.get(name) {
            if previous.rva != rva {
                self.anchors.remove(name);
            }
            return;
        }
        self.anchors.insert(
            name.into(),
            Anchor {
                rva,
            },
        );
    }
    pub fn event(&self, event: Event) -> Result<usize> {
        self.events
            .get(&event)
            .copied()
            .ok_or_else(|| Error::Jvm(format!("automap unresolved event {event:?}")))
    }
    pub fn hook(&self, module: &str, event: Event) -> Option<&Hook> {
        let slot = *self.events.get(&event)?;
        self.modules
            .get(module)?
            .hooks
            .iter()
            .find(|h| h.slot == slot)
    }
    pub fn signature(&mut self, image: &Image, name: &str, pattern: &str) {
        if let Ok(rva) = scan::unique(image, pattern) {
            self.insert(name, rva);
        }
    }
    pub fn require(&self, names: &[&str]) -> Result<()> {
        let missing: Vec<_> = names
            .iter()
            .filter(|name| !self.anchors.contains_key(**name))
            .copied()
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(Error::Jvm(format!(
                "automap unresolved: {}",
                missing.join(", ")
            )))
        }
    }
    pub fn setting(&self, module: &str, key: &str) -> Option<&Setting> {
        self.modules.get(module)?.settings.get(key)
    }
    pub fn alias_setting(&mut self, name: &str, module: &str, key: &str) {
        if let Some(setting) = self.setting(module, key) {
            self.insert(name, setting.descriptor);
        }
    }
}

impl Setting {
    pub fn descriptor_address(&self, map: &Map) -> usize {
        map.base + self.descriptor
    }
    pub fn value_address(&self, map: &Map) -> Result<usize> {
        self.value_offset
            .map(|offset| self.descriptor_address(map) + offset)
            .ok_or_else(|| Error::Jvm(format!("automap: unknown value layout for {}", self.key)))
    }
}
impl Module {
    pub fn enabled_address(&self, map: &Map) -> usize {
        map.base + self.descriptor + 0x14
    }
}
