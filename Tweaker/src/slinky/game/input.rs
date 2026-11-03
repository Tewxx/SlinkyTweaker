use super::*;

#[derive(Clone, Copy, Debug)]
pub enum Binding {
    Attack,
}
impl Binding {
    fn anchor(self) -> &'static str {
        match self {
            Self::Attack => "jni_attack_binding",
        }
    }
}

impl<'frame> Game<'frame> {
    pub fn key(&self, binding: Binding) -> Result<Key<'frame>> {
        let settings = self.object("jni_game_settings", self.minecraft)?;
        if settings.is_null() {
            return Err(Error::Jvm("native game settings are null".into()));
        }
        Key::new(self.object(binding.anchor(), settings)?)
            .ok_or_else(|| Error::Jvm(format!("native {binding:?} binding is null")))
    }
    pub fn key_pressed(&self, key: Key<'frame>) -> Result<bool> {
        self.boolean("native_key_pressed", key.raw)
    }
}
