use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=linker.ld");
    println!("cargo:rerun-if-changed=src/arch/x86_64/ap_trampoline.s");
    
    // Get output directory
    let out_dir = env::var("OUT_DIR").unwrap();
    
    // Assemble the AP trampoline using GNU assembler (gas syntax)
    let status = Command::new("as")
        .args([
            "--64",
            "-o", &format!("{}/ap_trampoline.o", out_dir),
            "src/arch/x86_64/ap_trampoline.s"
        ])
        .status()
        .expect("Failed to run as");
    
    if !status.success() {
        panic!("as failed to assemble ap_trampoline.s");
    }
    
    // Create a static library from the object file
    let status = Command::new("ar")
        .args([
            "crs",
            &format!("{}/libtrampoline.a", out_dir),
            &format!("{}/ap_trampoline.o", out_dir)
        ])
        .status()
        .expect("Failed to run ar");
    
    if !status.success() {
        panic!("ar failed to create libtrampoline.a");
    }
    
    // Tell cargo to link the trampoline library
    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=trampoline");
}