# RuState

RuStateは、Rustで実装された型安全なステートマシンおよびステートチャートライブラリです。XStateにインスパイアされており、モデルベーステスト（MBT）の原則に基づいた設計になっています。

## 概要

RuStateは以下の機能を提供します：

- ✅ 有限状態機械とステートチャート
- ✅ 階層状態
- ✅ 並列状態
- ✅ 遷移条件（ガード）
- ✅ アクション（副作用）
- ✅ コンテキスト（拡張状態）
- ✅ 型安全なAPI
- ✅ シリアライズ可能なマシン
- ✅ モデルベーステスト（MBT）サポート

## モデルベーステスト（MBT）の統合

RuStateは、モデルベーステストの原則を取り入れています：

1. **モデル定義**: 状態、遷移、ガード、アクションを使って、明示的なモデルを定義できます
2. **テストケース生成**: モデルから自動的にテストケースを生成
3. **テスト実行**: オンラインテストとオフラインテストの両方をサポート
4. **完全カバレッジ検証**: 全ての状態および遷移をカバーするテストの保証

### 主な機能

- **テスト生成器**: 状態マシンから自動的にテストケースを生成
- **オンラインテスト**: 実行時に状態マシンを直接テスト
- **オフラインテスト**: テストケースをエクスポートして後で実行可能
- **状態カバレッジレポート**: どの状態や遷移がテストされたかを確認

## 使用例

### シンプルな状態マシン

```rust
use rustate::{Action, ActionType, Machine, MachineBuilder, State, Transition};

// 状態の作成
let green = State::new("green");
let yellow = State::new("yellow");
let red = State::new("red");

// 遷移の作成
let green_to_yellow = Transition::new("green", "TIMER", "yellow");
let yellow_to_red = Transition::new("yellow", "TIMER", "red");
let red_to_green = Transition::new("red", "TIMER", "green");

// アクションの定義
let log_green = Action::new(
    "logGreen",
    ActionType::Entry,
    |_ctx, _evt| println!("Entering GREEN state - Go!"),
);

// マシンの構築
let mut machine = MachineBuilder::new("trafficLight")
    .state(green)
    .state(yellow)
    .state(red)
    .initial("green")
    .transition(green_to_yellow)
    .transition(yellow_to_red)
    .transition(red_to_green)
    .on_entry("green", log_green)
    .build()
    .unwrap();

// イベント送信
machine.send("TIMER").unwrap();
```

### モデルベーステスト例

```rust
use rustate::{Machine, TestGenerator, TestRunner};

// 既存の状態マシン定義から...
let machine = /* ... */;

// テストケース生成
let test_generator = TestGenerator::new(&machine);
let test_cases = test_generator.generate_all_transitions();

// テスト実行
let test_runner = TestRunner::new(&machine);
let results = test_runner.run_tests(test_cases);

// カバレッジレポート
let coverage = results.get_coverage();
println!("State coverage: {}%", coverage.state_coverage());
println!("Transition coverage: {}%", coverage.transition_coverage());
```

## インストール

Cargo.tomlに追加してください：

```toml
[dependencies]
rustate = "0.2.0"
```

## ドキュメント

### 主要概念

- **状態（State）**: ステートチャートのノードを表現
- **遷移（Transition）**: イベントに応じた状態間の移動を定義
- **ガード（Guard）**: 遷移条件を決定する論理
- **アクション（Action）**: 状態遷移中に実行される副作用
- **コンテキスト（Context）**: マシンの拡張状態を格納
- **テストジェネレータ（TestGenerator）**: モデルからテストケースを生成
- **テストランナー（TestRunner）**: テストケースを実行
- **カバレッジレポート（CoverageReport）**: テストのカバレッジを分析

## ロードマップ

- [ ] モデルチェッカーの統合
- [ ] プロパティベースのテスト
- [ ] テスト可視化ツール
- [ ] クイックチェックスタイルのテスト
- [ ] FuzzingによるMBT

## ライセンス

MIT 