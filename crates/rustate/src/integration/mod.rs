//! # RuState Integration
//! 
//! このモジュールはRuStateステートマシンをクレート間で型安全に統合するためのパターンを提供します。
//! 
//! ## 主な統合パターン
//! 
//! 1. **イベント転送パターン**: ステートマシン間でイベントを転送
//! 2. **コンテキスト共有パターン**: 共有コンテキストを使用したデータ連携
//! 3. **階層的統合パターン**: 親子関係を持つステートマシン間の連携

pub mod event_forwarding;
pub mod context_sharing;
pub mod hierarchical;

/// エラー型
pub mod error;
pub use error::{Error, Result, LockResultExt};

/// 再エクスポートして便利なインターフェースを提供
pub use event_forwarding::SharedMachineRef;
pub use context_sharing::SharedContext;
pub use hierarchical::ChildMachine; 