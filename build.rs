fn main() {
    println!("If the build fails pass the llvm lib folder to env LLVMLIB_DIR");
    let lib_dir = std::env::var_os("LLVMLIB_DIR");
    if lib_dir.is_some() {
        let reallib1 = lib_dir.unwrap_or_default();
        let reallib2 = reallib1.to_str().unwrap_or_default();
        println!("{}",format!("cargo:rustc-link-search={llvmLibs}", llvmLibs = reallib2));
    }
}
