fn main() {
    if std::env::var("TARGET").unwrap().contains("windows") {
        let java = std::env::var("JAVA_HOME").expect("JAVA_HOME not set");
        println!("cargo:rustc-link-search=native={java}/lib");
        println!("cargo:rustc-link-lib=jvm");
        println!("cargo:rustc-link-lib=psapi");
        println!("cargo:rustc-link-lib=opengl32");
    }
    println!("cargo:rerun-if-changed=vendor");
    cc::Build::new()
        .file("vendor/nanovg_glue.c")
        .file("vendor/nanovg/nanovg.c")
        .include("vendor")
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .flag_if_supported(format!("/d1trimfile:{}\\", std::env::var("CARGO_MANIFEST_DIR").unwrap()))
        .warnings(false)
        .compile("nanovg");
}
