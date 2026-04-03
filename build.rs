fn main() {
    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        println!("cargo:rustc-link-lib=msvcprt");
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        println!("cargo:rustc-link-lib=stdc++");
    }
}
