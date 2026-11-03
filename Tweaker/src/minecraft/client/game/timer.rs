use super::*;

impl Game {
    pub fn set_timer_speed(&mut self, speed: f32) -> Result<()> {
        self.scope(|game| unsafe {
            let mc = game.client.instance;
            let timer = game.object(
                mc,
                "timer",
                &["field_71428_T", "timer", "Y"],
                &["Lnet/minecraft/util/Timer;", "Lavl;"],
            )?;
            if timer.is_null() {
                return Err(Error::Jvm("Minecraft timer unavailable".into()));
            }
            let c = &game.client;
            let f = game.cache.field(
                c,
                timer,
                "timer_speed",
                &["field_74278_d", "timerSpeed", "d"],
                &["F"],
            )?;
            c.env.SetFloatField(timer, f, speed);
            Ok(())
        })
    }

    pub fn timer_reset_input(&mut self) -> Result<bool> {
        self.scope(|game| unsafe {
            let player = game.cache.player(&game.client)?;
            if player.is_null() {
                return Ok(false);
            }
            if game.button_down(0)? || game.button_down(1)? {
                return Ok(true);
            }
            let Some(held) = game.held_hotbar_key()? else {
                return Ok(false);
            };
            let c = &game.client;
            let inv = game.cache.field(
                c,
                player,
                "inventory",
                &["field_71071_by", "inventory", "bi"],
                &["Lnet/minecraft/entity/player/InventoryPlayer;", "Lwm;"],
            )?;
            let inventory = c.env.GetObjectField(player, inv);
            if inventory.is_null() {
                return Ok(false);
            }
            let slot = game.cache.field(
                c,
                inventory,
                "slot",
                &["field_70461_c", "currentItem", "c"],
                &["I"],
            )?;
            Ok(held != c.env.GetIntField(inventory, slot))
        })
    }

    unsafe fn button_down(&mut self, button: i32) -> Result<bool> {
        unsafe {
            let c = &self.client;
            let cls = self.cache.class(c, "mouse", &["org.lwjgl.input.Mouse"])?;
            let method =
                self.cache
                    .method(c, cls, "mouse_down", &["isButtonDown"], &["(I)Z"], true)?;
            let down = c.env.CallStaticBooleanMethod1(cls, method, button);
            exception::check(&c.env, "reading mouse button")?;
            Ok(down)
        }
    }

    unsafe fn key_down(&mut self, code: i32) -> Result<bool> {
        unsafe {
            if code == 0 || !(-100..=256).contains(&code) {
                return Ok(false);
            }
            if code < 0 {
                return self.button_down(code + 100);
            }
            let c = &self.client;
            let cls = self.cache.class(c, "keyboard", &["org.lwjgl.input.Keyboard"])?;
            let method = self
                .cache
                .method(c, cls, "key_down", &["isKeyDown"], &["(I)Z"], true)?;
            let down = c.env.CallStaticBooleanMethod1(cls, method, code);
            exception::check(&c.env, "reading key")?;
            Ok(down)
        }
    }

    unsafe fn held_hotbar_key(&mut self) -> Result<Option<i32>> {
        unsafe {
            let settings = self.game_settings()?;
            let c = &self.client;
            let f = self.cache.field(
                c,
                settings,
                "bindings",
                &["field_74324_K", "keyBindings", "ax"],
                &["[Lnet/minecraft/client/settings/KeyBinding;", "[Lavb;"],
            )?;
            let array = c.env.GetObjectField(settings, f);
            if array.is_null() {
                return Err(Error::Jvm("key bindings unavailable".into()));
            }
            let mut codes = [0i32; 9];
            for i in 0..c.env.GetArrayLength(array.cast()) {
                let c = &self.client;
                let key = c.env.GetObjectArrayElement(array.cast(), i);
                if key.is_null() {
                    continue;
                }
                let f = self.cache.field(
                    c,
                    key,
                    "key_description",
                    &["field_74515_c", "keyDescription", "d"],
                    &["Ljava/lang/String;"],
                )?;
                let name =
                    crate::jvm::strings::to_rust(&c.env, c.env.GetObjectField(key, f).cast());
                let Some(slot) = name
                    .strip_prefix("key.hotbar.")
                    .and_then(|n| n.parse::<usize>().ok())
                    .filter(|n| (1..=9).contains(n))
                else {
                    continue;
                };
                let f = self.cache.field(
                    c,
                    key,
                    "keycode",
                    &["field_74512_d", "keyCode", "g"],
                    &["I"],
                )?;
                codes[slot - 1] = c.env.GetIntField(key, f);
            }
            for (slot, code) in codes.into_iter().enumerate() {
                if self.key_down(code)? {
                    return Ok(Some(slot as i32));
                }
            }
            Ok(None)
        }
    }
}
