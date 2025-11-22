use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    println!("cargo:rerun-if-changed=../memory/src/spatial.zig");
    println!("cargo:rerun-if-changed=linker.ld");
    println!("cargo:rerun-if-changed=font.bin");

    compile_zig(&manifest_dir, &out_dir);
    generate_font_if_missing(&manifest_dir);

    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=spatial");
}

fn compile_zig(manifest_dir: &str, out_dir: &str) {
    let zig_source = PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("memory")
        .join("src")
        .join("spatial.zig");

    let obj_out = PathBuf::from(out_dir).join("spatial.o");
    let lib_out = PathBuf::from(out_dir).join("libspatial.a");

    println!("cargo:warning=Compiling Zig: {:?}", zig_source);

    // Use consistent flags - NO SSE to match kernel target
    let status = Command::new("zig")
        .args([
            "cc", "-c",
            zig_source.to_str().unwrap(),
            "-target", "x86_64-freestanding-none",
            "-mcmodel=kernel",
            "-O3",
            "-fno-stack-protector",
            "-fno-exceptions",
            "-fno-unwind-tables",
            "-fno-asynchronous-unwind-tables",
            "-mno-red-zone",
            "-mno-sse", "-mno-sse2", "-mno-mmx", "-mno-avx",
            "-msoft-float",
            "-fno-builtin",
            "-ffreestanding",
            "-nostdlib",
            "-o", obj_out.to_str().unwrap(),
        ])
        .status()
        .expect("Failed to run zig cc - is zig installed?");

    if !status.success() {
        panic!("Zig compilation failed");
    }

    // Create static library
    let status = Command::new("ar")
        .args(["crs", lib_out.to_str().unwrap(), obj_out.to_str().unwrap()])
        .status()
        .expect("Failed to run ar");

    if !status.success() {
        panic!("ar failed to create libspatial.a");
    }

    println!("cargo:warning=Built: {}", lib_out.display());
}

fn generate_font_if_missing(manifest_dir: &str) {
    let font_path = PathBuf::from(manifest_dir).join("font.bin");
    
    if font_path.exists() {
        let metadata = std::fs::metadata(&font_path).unwrap();
        if metadata.len() >= 95 * 16 {
            return; // Font exists and is valid size
        }
    }

    println!("cargo:warning=Generating basic font.bin");
    
    // Generate a basic 8x16 font for ASCII 32-126
    // This creates visible (if simple) characters
    let mut font = vec![0u8; 95 * 16];
    
    // Create simple block characters for visibility
    for char_idx in 0..95 {
        let offset = char_idx * 16;
        if char_idx == 0 {
            // Space - leave empty
            continue;
        }
        // Fill with a simple pattern based on character
        for row in 2..14 {
            font[offset + row] = 0x7E; // Basic filled rectangle
        }
    }
    
    std::fs::write(&font_path, font).expect("Failed to write font.bin");
}