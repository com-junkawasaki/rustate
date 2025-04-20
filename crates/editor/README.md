# Rustate Editor

WebAssembly-based editor for visualizing and editing Rustate state machines.

## 機能

- ステートマシン図の可視化
- ステート・遷移の追加・編集・削除
- JSON形式でのインポート・エクスポート
- ライブプレビュー機能
- Rustコード生成機能

## 使用方法

```rust
use rustate_editor::Editor;

fn main() {
    // エディタを初期化して表示
    Editor::new().render();
}
```

## ビルド方法

```bash
# WASMにビルド
wasm-pack build --target web

# 開発サーバーの起動
cd www && npm start
```