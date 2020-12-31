extern crate protoc_rust;
fn main() {
    protoc_rust::Codegen::new()
        .out_dir("./src/server/protos")
        .inputs(&["./protos/server_files.proto"])
        .run()
        .expect("protoc");
    // Note, this may fail sometimes so please use make-solution.sh
    println!("cargo:rustc-link-search=./faiss/c_api");
}
