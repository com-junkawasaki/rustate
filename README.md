# RuState

RuState is a type-safe state machine and statechart library implemented in Rust, inspired by XState and incorporating principles of model-based testing (MBT). It forms the core of a larger ecosystem designed for building, visualizing, testing, and deploying complex state-driven applications and agents.

## Demo

Check out the live demo: [RuState Demo](https://jun784.github.io/rustate/)

The demo features:
- Interactive state machine visualization
- Traffic light state machine example
- Hierarchical state machine example
- Real-time state transition tracking

## Project Structure & Ecosystem

The RuState project is organized as a Cargo workspace consisting of several crates:

- **`apps/rustate`**: The core state machine library. See [./apps/rustate/README.md](./apps/rustate/README.md) for details. (Status: Actively Developed)
- **`apps/editor`**: A web-based visual editor (WASM/Yew) for creating and visualizing RuState state machines. (Status: Work in Progress)
- **`apps/agent`**: Implements agent logic using RuState. (Status: Actively Developed)

## Model-Based Testing (MBT) Integration

*(Note: MBT features are planned but current status needs verification. Details previously here are moved to `apps/rustate/README.md`)*

## Roadmap

*(Based on current assessment)*

1.  **Foundation & Cleanup:**
    *   [ ] Run `cargo fmt`, `cargo clippy`, `cargo test --workspace` and fix issues.
2.  **Core Library (`rustate_core`) Enhancement:**
    *   [ ] Verify and potentially implement/enhance MBT features based on project goals.
    *   [ ] Refine API: Improve ergonomics based on usage in other crates.
    *   [ ] Documentation: Add comprehensive rustdoc comments and examples.
3.  **Editor (`editor`) Development:**
    *   [ ] Define MVP scope (e.g., visualize existing machine, basic node manipulation).
    *   [ ] Implement core visualization logic (using Yew/WASM).
    *   [ ] Implement loading/saving machines via `rustate_core` serialization.
4.  **Agent (`agent`) Development:**
    *   [ ] Refine agent architecture and integrate advanced features (e.g., LLM interaction).
    *   [ ] Improve storage and decision-making logic based on testing.
5.  **Demo (`demo`) Update:**
    *   [ ] Update demo to use the latest `rustate_core` features and API.
    *   [ ] Refresh the live demo deployment.
6.  **Testing & QA (`qa`):**
    *   [ ] Increase unit test coverage in `rustate_core` and `agent`.
    *   [ ] Add integration tests between `rustate_core` and other crates (`editor`, `demo`, `agent`).
    *   [ ] Develop comprehensive test suites, potentially including MBT if verified and implemented.
7.  **Documentation & Presentation:**
    *   [ ] Create crate interaction diagram for README.
    *   [ ] Update `docs/` directory with more detailed guides.
    *   [ ] Ensure `README.md` is fully consistent with the codebase.

*(Further steps depend on the resolution of foundational issues and priorities for `editor` and `agent` development.)*

## Installation

*(See `apps/rustate/README.md` for core library usage)*

```toml
# Example dependency (adjust based on publication status/version)
# [dependencies]
# rustate = { git = "https://github.com/jun784/rustate", branch = "main", package = "rustate" } 
```
*(Installation instructions need verification based on how crates are intended to be consumed)*

## Documentation

Refer to the individual crate READMEs for detailed documentation:
- [`apps/rustate/README.md`](./apps/rustate/README.md)
- *(Add links for editor, agent READMEs when created)*

## Getting Started

## License

MIT 

## プロジェクト完成度評価 (Project Completion Status)

### 機能実装状況 (Implementation Status)

| モジュール (Module)          | 進捗状況 (Progress) | 備考 (Notes)                                                                    |
|---------------------------|-------------------|---------------------------------------------------------------------------------| 
| `rustate_core`            | 95%               | 基本機能は実装済み、Actorパターン含む。最適化と拡張機能を追加中                      |
| `editor`                  | 80%               | 基本的な編集機能は実装済み、UI改善の余地あり                                            |
| `demo`                    | 85%               | トラフィックライト、階層状態機械の例は完成                                              |
| `agent`                   | 65%               | 主要機能実装中、LLM連携など高度化進行中                                                 |
| ドキュメント (Documentation)  | 75%               | 主要機能の説明はあるが、APIドキュメントの拡充が必要                                      |
| CI/CD                     | 70%               | GitHub Actionsで基本的なワークフロー設定済み                                             |

### 次のステップ (Next Steps)

1.  **Agent機能の実装完了**: LLMと統合したエージェントフレームワークの実装
2.  **Editor UI/UX改善**: より直感的で使いやすいインターフェースへ
3.  **ドキュメント拡充**: APIドキュメントの完成とxstateレベルの品質向上
4.  **テストカバレッジ向上**: 特に `agent`, `editor` クレートのカバレッジ向上
5.  **開発プロセス状態管理**: ビルド、デプロイなどの開発プロセスをrustateで制御する機能 (検討)

### 優先タスク (Priority Tasks)

- [ ] Agent機能の実装を加速
- [ ] Editor UI/UXの改善計画策定と実施
- [ ] テストカバレッジ向上のためのテストケース追加 (Agent, Editor)
- [ ] コアライブラリ (`rustate_core`) のAPIドキュメント拡充
- [ ] CI/CDパイプラインの強化 (リリースプロセス自動化など)

最終更新: 2024-07-29 *(Updated to current date)*

## Getting Started
