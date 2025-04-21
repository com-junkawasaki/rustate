# RuState Integration

RuStateステートマシンをクレート間で型安全に統合するためのパターン実装ライブラリです。

## 概要

このクレートは、RuStateステートマシンを複数のクレートにまたがって統合する方法を提供します。
3つの主要な統合パターンを実装しており、これらを組み合わせることで、型安全に複雑なステートマシンシステムを構築できます。

## 統合パターン

### 1. イベント転送パターン

ステートマシン間でイベントを転送するパターンです。`SharedMachineRef`を使って複数のクレート間でステートマシンへの参照を共有し、一方のステートマシンから他方のステートマシンにイベントを送信します。

```rust
use rustate_integration::SharedMachineRef;

// ステートマシンを共有参照でラップ
let shared_machine = SharedMachineRef::new(machine);

// 別のクレートに参照を渡して、そこからイベントを送信
shared_machine.send_event("EVENT_NAME")?;
```

### 2. コンテキスト共有パターン

複数のステートマシン間でコンテキストデータを共有するパターンです。`SharedContext`を使って、複数のクレートにまたがるステートマシンが同じコンテキストデータにアクセスできます。

```rust
use rustate_integration::SharedContext;

// 共有コンテキストを作成
let context = SharedContext::new();

// データを設定
context.set("key", "value")?;

// 別のクレートやステートマシンでデータを取得
let value = context.get::<String>("key")?;
```

### 3. 階層的統合パターン

親子関係を持つステートマシン間の連携パターンです。トレイトを使用して親ステートマシンが子ステートマシンと疎結合に連携できます。

```rust
use rustate_integration::{ChildMachine, hierarchical::DefaultChildMachine};
use std::sync::{Arc, Mutex};

// 子ステートマシンをトレイトで抽象化
let child_machine = DefaultChildMachine::new(machine, "final_state_id");
let child = Arc::new(Mutex::new(child_machine));

// 親ステートマシンから子ステートマシンを操作
if let Ok(mut child) = child.lock() {
    child.handle_parent_event("EVENT_NAME")?;
    
    if child.is_in_final_state() {
        // 完了処理
    }
}
```

## サンプルアプリケーション

`examples/combined_demo.rs`には、3つの統合パターンをすべて組み合わせた複合システムのデモが含まれています。
このデモでは、以下の要素を組み合わせています：

1. ワークフローステートマシン（全体の制御）
2. プロセスAステートマシン（コンテキスト共有パターン）
3. プロセスBステートマシン（階層的パターン）
4. コネクタマシン（イベント転送パターン）

```bash
# デモの実行方法
cargo run --example combined_demo
```

## 使用方法

Cargo.tomlに依存関係を追加：

```toml
[dependencies]
rustate = "0.2.0"
rustate-integration = "0.1.0"
```

基本的な使用例：

```rust
use rustate::{Machine, MachineBuilder, State, Transition};
use rustate_integration::{
    SharedMachineRef,
    SharedContext,
    ChildMachine,
    hierarchical::DefaultChildMachine,
};

// ステートマシンを作成
let machine = create_machine();

// クレート間で共有するための参照を作成
let shared_machine = SharedMachineRef::new(machine);

// 別のクレートに参照を渡して操作
shared_machine.send_event("EVENT")?;
```

## ライセンス

MIT 