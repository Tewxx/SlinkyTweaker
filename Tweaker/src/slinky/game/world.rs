use super::*;

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl<'frame> Game<'frame> {
    pub fn bounds(&self, entity: Entity<'frame>) -> Result<Bounds> {
        let object = self.object("native_bounding_box", entity.raw)?;
        if object.is_null() {
            return Err(Error::Jvm("native bounding box is null".into()));
        }
        let mut out = [f64::NAN; 6];
        native!(
            self,
            "native_bounds",
            unsafe extern "system" fn(*mut [f64; 6], Head, Obj) -> *mut [f64; 6],
            &mut out,
            self.head,
            object
        )?;
        if !out.iter().all(|v| v.is_finite()) || (0..3).any(|i| out[i] > out[i + 3]) {
            return Err(Error::Jvm("native bounding box is invalid".into()));
        }
        Ok(Bounds {
            min: [out[0], out[1], out[2]],
            max: [out[3], out[4], out[5]],
        })
    }
    pub fn crosshair_entity(&self) -> Result<Option<Entity<'frame>>> {
        let hit = self.object("native_object_mouse_over", self.minecraft)?;
        if hit.is_null() {
            return Ok(None);
        }
        Ok(Entity::new(self.object("native_hit_entity", hit)?))
    }
}
