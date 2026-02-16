use std::process::Command;

fn main() {
    // Compile GResource
    let out_dir = std::env::var("OUT_DIR").unwrap();
    
    // Check if glib-compile-resources is available
    let status = Command::new("glib-compile-resources")
        .args([
            "--sourcedir=data",
            &format!("--target={}/com.chrisdaggas.network-manager.gresource", out_dir),
            "data/com.chrisdaggas.network-manager.gresource.xml",
        ])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("cargo:rerun-if-changed=data/com.chrisdaggas.network-manager.gresource.xml");
            println!("cargo:rerun-if-changed=data/style.css");
            println!("cargo:rerun-if-changed=data/icons/");
            println!("cargo:rerun-if-changed=data/images/");
        }
        _ => {
            eprintln!("Warning: glib-compile-resources not found or failed. Resources will not be compiled.");
        }
    }

    // Tell cargo to rerun if any source files change
    println!("cargo:rerun-if-changed=src/");
}
