use super::*;
use jni_simple::{jtype, jtypes};

impl Game {
    pub fn right_click_block(&mut self, block: [i32; 3], side: i32, hit: [f64; 3]) -> Result<bool> {
        self.scope(|game| unsafe {
            let mc = game.client.instance;
            let player = game.cache.player(&game.client)?;
            let controller = game.object(
                mc,
                "player_controller",
                &["field_71442_b", "playerController", "c"],
                &[
                    "Lnet/minecraft/client/multiplayer/PlayerControllerMP;",
                    "Lbda;",
                ],
            )?;
            let world = game.object(
                mc,
                "world",
                &["field_71441_e", "theWorld", "f"],
                &["Lnet/minecraft/client/multiplayer/WorldClient;", "Lbdb;"],
            )?;
            if player.is_null() || controller.is_null() || world.is_null() {
                return Err(Error::Jvm("no player, controller or world to place with".into()));
            }
            let c = &game.client;
            let held = game.cache.object_method(
                c,
                player,
                "held_item",
                &["func_70694_bm", "getHeldItem", "bA"],
                &["()Lnet/minecraft/item/ItemStack;", "()Lzx;"],
            )?;
            let stack = c.env.CallObjectMethod0(player, held);
            exception::check(&c.env, "held item")?;

            let pos_class = game.cache.class(c, "block_pos", &["net.minecraft.util.BlockPos", "cj"])?;
            let pos_init = game
                .cache
                .method(c, pos_class, "block_pos_init", &["<init>"], &["(III)V"], false)?;
            let [x, y, z] = block;
            let pos = c.env.NewObjectA(pos_class, pos_init, jtypes!(x, y, z).as_ptr());
            let vec_class = game.cache.class(c, "vec3", &["net.minecraft.util.Vec3", "aui"])?;
            let vec_init = game
                .cache
                .method(c, vec_class, "vec3_init", &["<init>"], &["(DDD)V"], false)?;
            let [hx, hy, hz] = hit;
            let vec = c.env.NewObjectA(vec_class, vec_init, jtypes!(hx, hy, hz).as_ptr());
            let facing_class =
                game.cache.class(c, "facing", &["net.minecraft.util.EnumFacing", "cq"])?;
            let facing_of = game.cache.method(
                c,
                facing_class,
                "facing_of",
                &["func_82600_a", "getFront", "a"],
                &["(I)Lnet/minecraft/util/EnumFacing;", "(I)Lcq;"],
                true,
            )?;
            let facing = c.env.CallStaticObjectMethod1(facing_class, facing_of, side);
            exception::check(&c.env, "block position, face or hit")?;
            if pos.is_null() || vec.is_null() || facing.is_null() {
                return Err(Error::Jvm("could not build the click".into()));
            }

            let click = game.cache.object_method(
                c,
                controller,
                "right_click_block",
                &["func_178890_a", "onPlayerRightClick", "a"],
                &[
                    "(Lnet/minecraft/client/entity/EntityPlayerSP;\
                     Lnet/minecraft/client/multiplayer/WorldClient;\
                     Lnet/minecraft/item/ItemStack;Lnet/minecraft/util/BlockPos;\
                     Lnet/minecraft/util/EnumFacing;Lnet/minecraft/util/Vec3;)Z",
                    "(Lbew;Lbdb;Lzx;Lcj;Lcq;Laui;)Z",
                ],
            )?;
            let used = c.env.CallBooleanMethodA(
                controller,
                click,
                jtypes!(player, world, stack, pos, facing, vec).as_ptr(),
            );
            exception::check(&c.env, "right click on a block")?;
            if used {
                let swing = game.cache.object_method(
                    c,
                    player,
                    "swing_item",
                    &["func_71038_i", "swingItem", "bw"],
                    &["()V"],
                )?;
                c.env.CallVoidMethod0(player, swing);
                exception::check(&c.env, "swing")?;
            }
            Ok(used)
        })
    }
}
