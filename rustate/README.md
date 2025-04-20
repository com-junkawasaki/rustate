# Rustate: Rust State Machine Visualizer

Rust の状態機械 (state machine) を WASM と Yew を使って可視化するプロジェクトです。

## 目的

- `rustate` クレートで定義された状態機械を Web ブラウザ上でインタラクティブに表示・操作する。
- WASM と Yew を用いたフロントエンド開発の実践。

## プロジェクト構造

- `crates/rustate_core`: `rustate` を使用した状態機械のコアロジックを定義するクレート。
- `crates/rustate_visualizer`: Yew を使用した WASM フロントエンドアプリケーション。

## セットアップと実行

1.  **Rust と WASM ツールチェーンのインストール:**
    - Rust: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)
    - `wasm-pack`: `cargo install wasm-pack`
    - `trunk`: `cargo install trunk` (Yew アプリケーションのビルドと配信に使用)

2.  **ビルドと実行:**
    ```bash
    cd rustate/crates/rustate_visualizer
    trunk serve --open
    ```
    これにより、開発サーバーが起動し、ブラウザで `http://localhost:8080` などが開かれます。

## 進捗

- [x] プロジェクト構造の初期設定
- [x] `rustate_core` に簡単な状態機械 (信号機) を実装
- [x] `rustate_visualizer` に基本的な Yew アプリケーションを設定し、状態表示と遷移ボタンを実装
- [ ] 状態機械のグラフィカルな可視化 (例: SVG, Mermaid.js)
- [ ] 状態遷移履歴の表示
- [ ] より複雑な状態機械への対応

## 優先度リスト (ディレクトリ・ファイル)

- `crates/rustate_visualizer/src/lib.rs` (高): UI ロジックの中心。
- `crates/rustate_core/src/lib.rs` (中): 状態機械の定義。
- `crates/rustate_visualizer/static/index.html` (低): HTML の骨組み。
- `Cargo.toml` (各ファイル) (低): 依存関係定義。

## 今後の展望

- 状態と遷移を視覚的に表現する (グラフ描画ライブラリの利用)。
- ユーザーが異なる状態機械定義をロードできるようにする。