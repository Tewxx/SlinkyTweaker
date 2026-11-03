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
    pub fn on_ground(&self, entity: Entity<'frame>) -> Result<bool> {
        self.boolean("native_on_ground", entity.raw)
    }
    pub fn fall_distance(&self, entity: Entity<'frame>) -> Result<f32> {
        self.number("native_fall_distance", entity.raw)
    }
    pub fn sprinting(&self) -> Result<bool> {
        self.boolean("native_sprinting", self.player)
    }
    pub fn sneaking(&self) -> Result<bool> {
        self.boolean("native_sneaking", self.player)
    }
}
