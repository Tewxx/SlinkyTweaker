use super::*;

impl<'frame> Game<'frame> {
    pub fn crosshair_player(&self) -> Result<Option<Entity<'frame>>> {
        let Some(entity) = self.crosshair_entity()? else {
            return Ok(None);
        };
        let player = self.object("native_as_other_player", entity.raw)?;
        Ok(Entity::as_player(player))
    }
    pub fn tagged(&self, player: Entity<'frame>) -> Result<bool> {
        player.require_player()?;
        let value = native!(
            self,
            "native_player_tagged",
            unsafe extern "system" fn(Head, Obj, u32) -> u8,
            self.head,
            player.raw,
            7
        )?;
        Ok(value != 0)
    }
    pub fn holding_weapon(&self) -> Result<(bool, bool)> {
        let mut flags = 0u32;
        let weapon = native!(
            self,
            "native_holding_weapon",
            unsafe extern "system" fn(Head, Obj, *mut u32) -> u8,
            self.head,
            self.player,
            &mut flags
        )?;
        Ok((weapon != 0, flags & 2 != 0))
    }
    pub fn displace_yaw(&self, target: Entity<'frame>) -> Result<f32> {
        let yaw = native!(
            self,
            "native_displace_yaw",
            unsafe extern "system" fn(Obj, Obj) -> f32,
            self.event,
            target.raw
        )?;
        if yaw.is_finite() {
            Ok(yaw)
        } else {
            Err(Error::Jvm("displace yaw not finite".into()))
        }
    }
}
