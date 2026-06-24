//! TypeScript バインディング (../src/bindings.ts) を生成する補助バイナリ。
//! 使い方: `cargo run --bin gen_bindings`
fn main() {
    ankiros_lib::export_typescript_bindings();
    println!("bindings exported to ../src/bindings.ts");
}
