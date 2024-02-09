extern crate pkg_config;

fn main() {
    println!("cargo:include=/usr/include/python3.10");
    println!("cargo:rustc-link-arg=-lpython3.10");
}

