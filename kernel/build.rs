use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=linker.ld");
    println!("cargo:rerun-if-changed=SpaceMono-Regular.ttf");
    
    // Verify font exists
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let font_path = PathBuf::from(&manifest_dir).join("SpaceMono-Regular.ttf");
    
    if !font_path.exists() {
        panic!(
            "Font file not found: {}\nPlease place SpaceMono-Regular.ttf in the kernel directory.",
            font_path.display()
        );
    }
    
    // Verify font is valid by checking minimum size (TTF files are at least 12 bytes)
    let metadata = std::fs::metadata(&font_path)
        .expect("Failed to read font file metadata");
    
    if metadata.len() < 12 {
        panic!("Font file is too small to be a valid TTF: {} bytes", metadata.len());
    }
    
    println!("cargo:rustc-env=FONT_PATH={}", font_path.display());
    println!("Font validated: {} ({} bytes)", font_path.display(), metadata.len());
}