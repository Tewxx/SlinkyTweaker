use super::game::Game;
use super::*;
use jni_simple::{jtype, jtypes};
use std::cell::RefCell;

pub struct Icon {
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub state: u16,
}

impl Game {
    pub fn draw_block_icons(&mut self, icons: &[Icon], [width, height]: [f32; 2]) -> Result<()> {
        if icons.is_empty() {
            return Ok(());
        }
        self.scope(|game| unsafe {
            let c = &game.client;
            let state = State::new(c, &mut game.cache)?;
            let field = game.cache.field(
                c,
                c.instance,
                "render_item",
                &["field_175621_X", "renderItem", "ab"],
                &[
                    "Lnet/minecraft/client/renderer/entity/RenderItem;",
                    "Lbjh;",
                ],
            )?;
            let renderer = c.env.GetObjectField(c.instance, field);
            if renderer.is_null() {
                return Ok(());
            }
            let render = game.cache.object_method(
                c,
                renderer,
                "render_item_gui",
                &["func_180450_b", "renderItemAndEffectIntoGUI", "b"],
                &["(Lnet/minecraft/item/ItemStack;II)V", "(Lzx;II)V"],
            )?;
            let mut stacks = Vec::with_capacity(icons.len());
            for icon in icons {
                stacks.push(stack(c, &mut game.cache, icon.state)?);
            }

            let saved = gl::Saved::capture();
            gl::glMatrixMode(gl::PROJECTION);
            gl::glPushMatrix();
            gl::glLoadIdentity();
            gl::glOrtho(0.0, width as f64, height as f64, 0.0, 1000.0, 3000.0);
            gl::glMatrixMode(gl::MODELVIEW);
            gl::glPushMatrix();
            gl::glLoadIdentity();
            gl::glTranslatef(0.0, 0.0, -2000.0);
            gl::glDepthMask(1);
            gl::glClear(gl::DEPTH_BUFFER_BIT);
            state.call(c, State::ENABLE_TEXTURE);
            gl::glEnable(gl::TEXTURE_2D);
            state.call(c, State::ENABLE_DEPTH);
            state.call(c, State::ENABLE_BLEND);
            state.blend_func(c, gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::glBlendFunc(gl::SRC_ALPHA as u32, gl::ONE_MINUS_SRC_ALPHA as u32);
            gl::glEnable(gl::RESCALE_NORMAL);
            c.env.CallStaticVoidMethod0(state.lighting, state.gui_lighting);
            let mut result = exception::check(&c.env, "Bed Plates item lighting");
            for (icon, &stack) in icons.iter().zip(&stacks) {
                if result.is_err() || stack.is_null() {
                    continue;
                }
                let k = icon.size / 16.0;
                gl::glPushMatrix();
                gl::glTranslatef(icon.x, icon.y, 0.0);
                gl::glScalef(k, k, 1.0);
                c.env.CallVoidMethodA(renderer, render, jtypes!(stack, 0i32, 0i32).as_ptr());
                gl::glPopMatrix();
                result = exception::check(&c.env, "Bed Plates item icon");
            }
            c.env.CallStaticVoidMethod0(state.lighting, state.no_lighting);
            exception::check(&c.env, "Bed Plates item lighting")?;
            gl::glDisable(gl::RESCALE_NORMAL);
            gl::glClear(gl::DEPTH_BUFFER_BIT);
            gl::glMatrixMode(gl::PROJECTION);
            gl::glPopMatrix();
            gl::glMatrixMode(gl::MODELVIEW);
            gl::glPopMatrix();
            saved.restore(|id| state.call(c, id), |s, d| state.blend_func(c, s, d));
            result
        })
    }
}

thread_local! {
    static STACKS: RefCell<Vec<(u16, jobject)>> = const { RefCell::new(Vec::new()) };
}

unsafe fn stack(c: &Client, cache: &mut cache::Cache, state: u16) -> Result<jobject> {
    if let Some(stack) = STACKS.with(|s| s.borrow().iter().find(|s| s.0 == state).map(|s| s.1)) {
        return Ok(stack);
    }
    unsafe {
        let block_class = cache.class(c, "block", &["net.minecraft.block.Block", "afh"])?;
        let by_id = cache.method(
            c,
            block_class,
            "block_by_id",
            &["func_149729_e", "getBlockById", "c"],
            &["(I)Lnet/minecraft/block/Block;", "(I)Lafh;"],
            true,
        )?;
        let stack_class = cache.class(c, "item_stack", &["net.minecraft.item.ItemStack", "zx"])?;
        let init = cache.method(
            c,
            stack_class,
            "item_stack_block",
            &["<init>"],
            &["(Lnet/minecraft/block/Block;II)V", "(Lafh;II)V"],
            false,
        )?;
        let block = c.env.CallStaticObjectMethod1(block_class, by_id, (state >> 4) as i32);
        exception::check(&c.env, "Bed Plates block lookup")?;
        if block.is_null() {
            return Ok(std::ptr::null_mut());
        }
        let local = c
            .env
            .NewObjectA(stack_class, init, jtypes!(block, 1i32, (state & 15) as i32).as_ptr());
        exception::check(&c.env, "Bed Plates item stack")?;
        c.env.DeleteLocalRef(block);
        let global = if local.is_null() { local } else { c.env.NewGlobalRef(local) };
        c.env.DeleteLocalRef(local);
        STACKS.with(|s| s.borrow_mut().push((state, global)));
        Ok(global)
    }
}

struct State {
    manager: jclass,
    ids: [jmethodID; 6],
    blend: jmethodID,
    lighting: jclass,
    gui_lighting: jmethodID,
    no_lighting: jmethodID,
}
impl State {
    const ENABLE_TEXTURE: usize = 0;
    const DISABLE_TEXTURE: usize = 1;
    const ENABLE_DEPTH: usize = 2;
    const DISABLE_DEPTH: usize = 3;
    const ENABLE_BLEND: usize = 4;
    const DISABLE_BLEND: usize = 5;

    unsafe fn new(c: &Client, cache: &mut cache::Cache) -> Result<Self> {
        unsafe {
            let manager = cache.class(
                c,
                "gl_state_manager",
                &["net.minecraft.client.renderer.GlStateManager", "bfl"],
            )?;
            let mut method = |key, names: &[&str], sig| cache.method(c, manager, key, names, &[sig], true);
            let ids = [
                method("gsm_enable_texture", &["func_179098_w", "enableTexture2D", "w"], "()V")?,
                method("gsm_disable_texture", &["func_179090_x", "disableTexture2D", "x"], "()V")?,
                method("gsm_enable_depth", &["func_179126_j", "enableDepth", "j"], "()V")?,
                method("gsm_disable_depth", &["func_179097_i", "disableDepth", "i"], "()V")?,
                method("gsm_enable_blend", &["func_179147_l", "enableBlend", "l"], "()V")?,
                method("gsm_disable_blend", &["func_179084_k", "disableBlend", "k"], "()V")?,
            ];
            let blend = method("gsm_blend_func", &["func_179112_b", "blendFunc", "b"], "(II)V")?;
            let lighting = cache.class(
                c,
                "render_helper",
                &["net.minecraft.client.renderer.RenderHelper", "avc"],
            )?;
            let gui_lighting = cache.method(
                c,
                lighting,
                "gui_item_lighting",
                &["func_74520_c", "enableGUIStandardItemLighting", "c"],
                &["()V"],
                true,
            )?;
            let no_lighting = cache.method(
                c,
                lighting,
                "no_item_lighting",
                &["func_74518_a", "disableStandardItemLighting", "a"],
                &["()V"],
                true,
            )?;
            Ok(Self {
                manager,
                ids,
                blend,
                lighting,
                gui_lighting,
                no_lighting,
            })
        }
    }
    unsafe fn call(&self, c: &Client, id: usize) {
        unsafe { c.env.CallStaticVoidMethod0(self.manager, self.ids[id]) }
    }
    unsafe fn blend_func(&self, c: &Client, src: i32, dst: i32) {
        unsafe { c.env.CallStaticVoidMethod2(self.manager, self.blend, src, dst) }
    }
}

mod gl {
    pub const PROJECTION: u32 = 0x1701;
    pub const MODELVIEW: u32 = 0x1700;
    pub const DEPTH_BUFFER_BIT: u32 = 0x100;
    pub const TEXTURE_2D: u32 = 0x0DE1;
    pub const DEPTH_TEST: u32 = 0x0B71;
    pub const BLEND: u32 = 0x0BE2;
    pub const RESCALE_NORMAL: u32 = 0x803A;
    pub const SRC_ALPHA: i32 = 0x0302;
    pub const ONE_MINUS_SRC_ALPHA: i32 = 0x0303;
    const DEPTH_WRITEMASK: u32 = 0x0B72;
    const BLEND_SRC: u32 = 0x0BE1;
    const BLEND_DST: u32 = 0x0BE0;

    #[link(name = "opengl32")]
    extern "system" {
        pub fn glMatrixMode(mode: u32);
        pub fn glPushMatrix();
        pub fn glPopMatrix();
        pub fn glLoadIdentity();
        pub fn glOrtho(l: f64, r: f64, b: f64, t: f64, n: f64, f: f64);
        pub fn glTranslatef(x: f32, y: f32, z: f32);
        pub fn glScalef(x: f32, y: f32, z: f32);
        pub fn glDepthMask(flag: u8);
        pub fn glClear(mask: u32);
        pub fn glEnable(cap: u32);
        pub fn glDisable(cap: u32);
        pub fn glBlendFunc(src: u32, dst: u32);
        fn glIsEnabled(cap: u32) -> u8;
        fn glGetBooleanv(name: u32, out: *mut u8);
        fn glGetIntegerv(name: u32, out: *mut i32);
    }

    pub struct Saved {
        texture: bool,
        depth: bool,
        blend: bool,
        depth_mask: u8,
        blend_func: [i32; 2],
    }
    impl Saved {
        pub unsafe fn capture() -> Self {
            unsafe {
                let mut depth_mask = 1;
                let mut blend_func = [SRC_ALPHA, ONE_MINUS_SRC_ALPHA];
                glGetBooleanv(DEPTH_WRITEMASK, &mut depth_mask);
                glGetIntegerv(BLEND_SRC, &mut blend_func[0]);
                glGetIntegerv(BLEND_DST, &mut blend_func[1]);
                Self {
                    texture: glIsEnabled(TEXTURE_2D) != 0,
                    depth: glIsEnabled(DEPTH_TEST) != 0,
                    blend: glIsEnabled(BLEND) != 0,
                    depth_mask,
                    blend_func,
                }
            }
        }
        pub unsafe fn restore(&self, mut toggle: impl FnMut(usize), blend: impl FnOnce(i32, i32)) {
            use super::State;
            toggle(if self.texture { State::ENABLE_TEXTURE } else { State::DISABLE_TEXTURE });
            toggle(if self.depth { State::ENABLE_DEPTH } else { State::DISABLE_DEPTH });
            toggle(if self.blend { State::ENABLE_BLEND } else { State::DISABLE_BLEND });
            blend(self.blend_func[0], self.blend_func[1]);
            unsafe {
                for (on, cap) in [(self.texture, TEXTURE_2D), (self.depth, DEPTH_TEST), (self.blend, BLEND)] {
                    if on { glEnable(cap) } else { glDisable(cap) }
                }
                glBlendFunc(self.blend_func[0] as u32, self.blend_func[1] as u32);
                glDepthMask(self.depth_mask);
            }
        }
    }
}

