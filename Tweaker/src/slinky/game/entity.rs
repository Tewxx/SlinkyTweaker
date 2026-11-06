use super::*;

impl<'frame> Game<'frame> {
    pub fn position(&self, entity: Entity<'frame>) -> Result<[f64; 3]> {
        self.vector("native_position", entity.raw)
    }
    pub fn previous_position(&self, entity: Entity<'frame>) -> Result<[f64; 3]> {
        self.vector("native_prev_position", entity.raw)
    }
    pub fn entity_id(&self, entity: Entity<'frame>) -> Result<i32> {
        self.integer("native_entity_id", entity.raw)
    }
    pub fn sprinting(&self) -> Result<bool> {
        self.boolean("native_sprinting", self.player)
    }
}
