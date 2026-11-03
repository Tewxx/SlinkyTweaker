use super::*;

impl Game {
    pub(super) unsafe fn game_settings(&mut self) -> Result<jobject> {
        unsafe {
            let mc = self.client.instance;
            let settings = self.object(
                mc,
                "game_settings",
                &["field_71474_y", "gameSettings", "t"],
                &["Lnet/minecraft/client/settings/GameSettings;", "Lavh;"],
            )?;
            if settings.is_null() {
                return Err(Error::Jvm("GameSettings unavailable".into()));
            }
            Ok(settings)
        }
    }

}
