/*!
Rustate Agent - ステートマシン駆動の LLM エージェントフレームワーク

このクレートは、rustateを使用してステートマシンに基づく
高度なAIエージェントを構築するためのフレームワークを提供します。
エージェントは観測、フィードバック、洞察を通じて学習し、
構造化された決定を行います。
*/

pub mod agent;
pub mod decision;
pub mod episode;
pub mod error;
pub mod feedback;
pub mod goal;
pub mod insight;
pub mod observation;
pub mod policy;
pub mod storage;

// 再エクスポート
pub use agent::Agent;
pub use decision::{Decision, DecisionMaker};
pub use episode::Episode;
pub use error::AgentError;
pub use feedback::{Feedback, FeedbackProvider};
pub use goal::Goal;
pub use insight::Insight;
pub use observation::Observation;
pub use policy::Policy;
pub use storage::Storage;

/// ライブラリ内のすべての公開項目をひとつの場所からアクセスできるようにします
pub mod prelude {
    pub use crate::agent::Agent;
    pub use crate::decision::{Decision, DecisionMaker};
    pub use crate::episode::Episode;
    pub use crate::error::{AgentError, Result};
    pub use crate::feedback::{Feedback, FeedbackProvider};
    pub use crate::goal::Goal;
    pub use crate::insight::Insight;
    pub use crate::observation::Observation;
    pub use crate::policy::Policy;
    pub use crate::storage::Storage;

    // rustate の必要な型も再エクスポートする
    pub use rustate::{Context, Event, EventTrait, Machine, State, StateTrait};

    // rustate integration の型も再エクスポートする
    #[cfg(feature = "integration")]
    pub use rustate::integration::{
        context_sharing, event_forwarding, hierarchical, ChildMachine, SharedContext,
        SharedMachineRef,
    };
}

// ユーティリティ関数
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
