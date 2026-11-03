use super::*;

impl crate::slinky::hotbar::Access for Game<'_> {
    fn current_item(&self) -> Result<i32> {
        self.current_slot()
    }
    fn select(&self, slot: i32) -> Result<()> {
        self.select_slot(slot)
    }
}

impl<'frame> Game<'frame> {
    fn inventory(&self) -> Result<Obj> {
        let inventory = self.object("native_inventory", self.player)?;
        if inventory.is_null() {
            Err(Error::Jvm("native inventory is null".into()))
        } else {
            Ok(inventory)
        }
    }
    pub fn current_slot(&self) -> Result<i32> {
        let slot = self.integer("native_current_item", self.inventory()?)?;
        if (0..9).contains(&slot) {
            Ok(slot)
        } else {
            Err(Error::Jvm(format!("invalid hotbar slot {slot}")))
        }
    }
    pub fn select_slot(&self, slot: i32) -> Result<()> {
        if !(0..9).contains(&slot) {
            return Err(Error::Jvm("hotbar slot must be in 0..=8".into()));
        }
        native!(
            self,
            "native_set_current_item",
            unsafe extern "system" fn(Head, Obj, i32),
            self.head,
            self.inventory()?,
            slot
        )
    }
    pub fn hotbar(&self, slot: i32) -> Result<Option<Stack<'frame>>> {
        if !(0..9).contains(&slot) {
            return Err(Error::Jvm("hotbar slot must be in 0..=8".into()));
        }
        let stack = native!(
            self,
            "native_stack_in_slot",
            unsafe extern "system" fn(Head, Obj, i32) -> Obj,
            self.head,
            self.inventory()?,
            slot
        )?;
        Ok(Stack::new(stack))
    }
    pub fn stack_item(&self, stack: Stack<'frame>) -> Result<Option<Item<'frame>>> {
        Ok(Item::new(self.object("native_stack_item", stack.raw)?))
    }
    pub fn stack_size(&self, stack: Stack<'frame>) -> Result<i32> {
        self.integer("native_stack_size", stack.raw)
    }
    pub fn block_from_item(&self, item: Item<'frame>) -> Result<Option<Block<'frame>>> {
        Ok(Block::new(self.object("native_block_from_item", item.raw)?))
    }
    pub fn hotbar_block(&self, slot: i32) -> Result<Option<(i32, i32)>> {
        let Some(stack) = self.hotbar(slot)? else {
            return Ok(None);
        };
        let Some(item) = self.stack_item(stack)? else {
            return Ok(None);
        };
        let Some(block) = self.block_from_item(item)? else {
            return Ok(None);
        };
        Ok(Some((self.block_id(block)?, self.stack_size(stack)?)))
    }
}
