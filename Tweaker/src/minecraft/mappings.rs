use crate::jvm::Method;

pub const MINECRAFT_CLASS_CANDIDATES: &[&str] =
    &["ave", "net.minecraft.client.Minecraft", "bib", "bao"];

pub const GET_MINECRAFT: Method = Method {
    what: "Minecraft.getMinecraft()",
    names: &["func_71410_x", "getMinecraft", "A"],
    sigs: &["()Lnet/minecraft/client/Minecraft;", "()Lave;"],
};
