fn main() {
    let sdk_path = std::path::Path::new("decklink_sdk");
    let mut build = cc::Build::new();
    build.cpp(true);
    
    if sdk_path.join("DeckLinkAPI_h.h").exists() {
        println!("cargo:warning=M-PLAYLIST: Blackmagic SDK detected. Building native SDI shim.");
        build.include(sdk_path)
             .file("src/decklink_bridge.cpp")
             .file("decklink_sdk/DeckLinkAPI_i.c")
             .compile("decklink");
    } else {
        println!("cargo:warning=M-PLAYLIST: Blackmagic SDK missing. Building dummy SDI shim.");
        build.file("src/decklink_dummy.cpp").compile("decklink");
    }
    println!("cargo:rustc-link-lib=Ole32");
    println!("cargo:rustc-link-lib=OleAut32");
    println!("cargo:rerun-if-changed=src/decklink_bridge.cpp");
    println!("cargo:rerun-if-changed=src/decklink_dummy.cpp");
}
