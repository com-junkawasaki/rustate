mod action;
mod context;
mod error;
mod event;
mod guard;
pub mod machine;
pub mod state;
#[cfg(any(feature = "mbt", feature = "property-testing"))]
mod test;
pub mod transition;

#[cfg(feature = "wasm")]
mod wasm;

#[cfg(feature = "wasm")]
pub use wasm::*;

#[cfg(feature = "codegen")]
mod codegen;

#[cfg(feature = "codegen")]
pub use codegen::*;

/// # 統合パターン機能
///
/// このモジュールは複数のクレートにまたがるステートマシン間で
/// 型安全な連携を実現するためのパターンを提供します。
///
/// ## 主な統合パターン
///
/// - **イベント転送**: 複数のステートマシン間でイベントを転送
/// - **コンテキスト共有**: 共有データを使用してステートマシン間で状態を連携
/// - **階層的統合**: 親子関係を持つステートマシン間の連携
///
/// ## 使用方法
///
/// Cargo.tomlに以下を追加して統合機能を有効化します：
///
/// ```toml
/// [dependencies]
/// rustate = { version = "0.2.4", features = ["integration"] }
/// ```
///
/// 非同期機能を使用する場合は以下のように指定します：
///
/// ```toml
/// [dependencies]
/// rustate = { version = "0.2.4", features = ["integration_async"] }
/// ```
///
/// 詳細な使用例は `integration` モジュールのドキュメントを参照してください。
#[cfg(feature = "integration")]
pub mod integration;

/// # ネットワーク連携機能
///
/// RuStateのステートマシンをネットワーク越しに制御・監視するための機能は
/// 別クレート `rustate-grpc` で提供されています。
///
/// ## 主なネットワーク連携機能
///
/// - **リモートステートマシン制御**: gRPC経由でステートマシンを作成・操作
/// - **リアルタイム状態監視**: ストリーミングによる状態変化の監視
/// - **複数言語サポート**: Protocol Buffersによる異なる言語間の連携
///
/// ## 使用方法
///
/// Cargo.tomlに以下を追加してgRPC機能を有効化します：
///
/// ```toml
/// [dependencies]
/// rustate-grpc = { version = "0.1.0", features = ["full"] }
/// ```
///
/// 詳細は `rustate-grpc` クレートのドキュメントを参照してください。
pub use action::{Action, ActionType, IntoAction};
pub use context::Context;
pub use error::{Result, StateError as Error};
pub use event::{Event, EventTrait, IntoEvent};
pub use guard::{Guard, IntoGuard};
pub use machine::{Machine, MachineBuilder};
pub use state::{State, StateTrait, StateType};
pub use transition::Transition;

// モデルベーステストの機能をエクスポート
#[cfg(any(feature = "mbt", feature = "property-testing"))]
pub use test::{
    CoverageReport, ModelChecker, Property, PropertyType, TestCase, TestGenerator, TestResult,
    TestResults, TestRunner, VerificationResult,
};

// Property-basedテスト機能をエクスポート
#[cfg(feature = "property-testing")]
pub use test::{
    EventSequenceStrategyBuilder, PropertyTestResult, PropertyTestRunner, StateMachineProperty,
};

/// # 統合パターンの主要コンポーネント
///
/// これらは複数のクレートにまたがるステートマシン間の連携を容易にするための
/// 便利なインターフェースです。
///
/// - `SharedMachineRef`: ステートマシンへの共有参照（イベント転送パターン）
/// - `SharedContext`: ステートマシン間で共有されるコンテキスト（コンテキスト共有パターン）
/// - `ChildMachine`: 親子関係を持つステートマシン間の連携トレイト（階層的統合パターン）
#[cfg(feature = "integration")]
pub use integration::{
    context_sharing::SharedContext, event_forwarding::SharedMachineRef, hierarchical::ChildMachine,
    Error as IntegrationError, Result as IntegrationResult,
};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
