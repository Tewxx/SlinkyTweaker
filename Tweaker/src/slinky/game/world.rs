use super::*;

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub block: [i32; 3],
    pub side: i32,
}

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
    pub fn block_at(&self, position: [i32; 3]) -> Result<Option<Block<'frame>>> {
        let [x, y, z] = position;
        let object = native!(
            self,
            "native_block_at",
            unsafe extern "system" fn(Head, Obj, i32, i32, i32) -> Obj,
            self.head,
            self.world,
            x,
            y,
            z
        )?;
        Ok(Block::new(object))
    }
    pub fn block_id(&self, block: Block<'frame>) -> Result<i32> {
        self.integer("native_block_id", block.raw)
    }
    pub fn block_id_at(&self, position: [i32; 3]) -> Result<Option<i32>> {
        self.block_at(position)?
            .map(|block| self.block_id(block))
            .transpose()
    }
    pub fn eye(&self) -> Result<[f64; 3]> {
        let [x, y, z] = self.position(self.player())?;
        Ok([x, y + if self.sneaking()? { 1.54 } else { 1.62 }, z])
    }
    pub fn ray(&self, from: [f64; 3], to: [f64; 3]) -> Result<Option<Hit>> {
        if !from.iter().chain(to.iter()).all(|v| v.is_finite()) {
            return Err(Error::Jvm("ray points must be finite".into()));
        }
        let a = native!(
            self,
            "native_vec3_new",
            unsafe extern "system" fn(Head, *const [f64; 3]) -> Obj,
            self.head,
            &from
        )?;
        let b = native!(
            self,
            "native_vec3_new",
            unsafe extern "system" fn(Head, *const [f64; 3]) -> Obj,
            self.head,
            &to
        )?;
        if a.is_null() || b.is_null() {
            return Err(Error::Jvm("native ray vectors are null".into()));
        }
        let hit = native!(
            self,
            "native_ray_trace",
            unsafe extern "system" fn(Head, Obj, Obj, Obj, u8, u8, u8) -> Obj,
            self.head,
            self.world,
            a,
            b,
            0,
            0,
            0
        )?;
        if hit.is_null() {
            return Ok(None);
        }
        let mut block = [i32::MIN; 3];
        native!(
            self,
            "native_hit_block",
            unsafe extern "system" fn(*mut [i32; 3], Head, Obj) -> *mut [i32; 3],
            &mut block,
            self.head,
            hit
        )?;
        let side = self.integer("native_hit_side", hit)?;
        if !(0..6).contains(&side) || block.contains(&i32::MIN) {
            return Err(Error::Jvm("native ray hit is invalid".into()));
        }
        Ok(Some(Hit { block, side }))
    }
    pub fn crosshair_entity(&self) -> Result<Option<Entity<'frame>>> {
        let hit = self.object("native_object_mouse_over", self.minecraft)?;
        if hit.is_null() {
            return Ok(None);
        }
        Ok(Entity::new(self.object("native_hit_entity", hit)?))
    }
}
