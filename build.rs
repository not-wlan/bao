fn main() {
    println!("If the build fails pass the llvm lib folder to env LLVMLIB_DIR");
    if let Ok(name) = std::env::var("LLVMLIB_DIR") {
        println!("cargo:rustc-link-search={}", name);
    }
}
