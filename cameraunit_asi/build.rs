extern crate bindgen;
extern crate cc;

use std::path::PathBuf;

use bindgen::CargoCallbacks;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const LIB_PATH: &str = "lib/x64";

#[cfg(all(target_os = "linux", target_arch = "x86"))]
const LIB_PATH: &str = "lib/x86";

#[cfg(all(target_os = "linux", target_arch = "arm"))]
const LIB_PATH: &str = "lib/armv7";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const LIB_PATH: &str = "lib/armv8";

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const LIB_PATH: &str = "lib/mac";

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const LIB_PATH: &str = "lib/mac";

fn main() {
    // This is the directory where the `c` library is located.
    // Canonicalize the path as `rustc-link-search` requires an absolute path.
    let libdir_path = PathBuf::from(LIB_PATH)
        .canonicalize()
        .expect("cannot canonicalize path");

    // This is the path to the `c` headers file.
    let headers_path = libdir_path.join("../../include").join("ASICamera2.h");

    let headers_path_str = headers_path.to_str()
        .expect("Path is not a valid string");

    // Tell cargo to look for shared libraries in the specified directory
    println!("cargo:rustc-link-search={}", libdir_path.to_str().unwrap());

    // Tell cargo to tell rustc to link our `hello` library. Cargo will
    // automatically know it must look for a `libhello.a` file.
    println!("cargo:rustc-link-lib=ASICamera2");
    println!("cargo:rustc-link-lib=pthread");
    println!("cargo:rustc-link-lib=rt");
    println!("cargo:rustc-link-lib=m");
    println!("cargo:rustc-link-lib=usb-1.0");

    // Tell cargo to invalidate the built crate whenever the header changes.
    println!("cargo:rerun-if-changed={}", headers_path_str);

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header(headers_path_str)
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from("src/asicamera_2/");
    if !out_path.exists() {
        std::fs::create_dir(&out_path).expect("Failed to create output directory");
    }
    let out_path = out_path
        .join("asicamera2_bindings.rs");

    bindings
        .write_to_file(out_path)
        .expect("Couldn't write bindings!");

    // cc::Build::new()
    //     .file("src/lib.c")
    //     .compile("cameraunit_asi");
}