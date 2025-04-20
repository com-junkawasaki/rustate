use rustate_editor::Editor;

fn main() {
    // WASMをビルドする場合は何もしないが、
    // ネイティブでデバッグ実行するためのエントリーポイント
    println!("This example is meant to be built with wasm-pack.");
    println!("Run: wasm-pack build --target web");
}