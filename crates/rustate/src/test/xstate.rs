use crate::{Context, Event, EventTrait, IntoEvent, Machine, Result, StateTrait};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::marker::PhantomData;

/// XState互換のテストモデル
#[derive(Debug, Clone)]
pub struct XStateTestModel<S, E>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static,
{
    /// ステートマシン
    machine: Machine<S, E>,
    /// 実装プロバイダー
    implementations: HashMap<String, Box<dyn Fn(&mut Context, &Event) -> Result<()> + 'static>>,
    /// テスト固有のアサーション
    assertions: HashMap<String, Box<dyn Fn(&Machine<S, E>) -> bool + 'static>>,
    /// 検証済みの経路
    verified_paths: HashSet<String>,
}

/// テストプランを表現する構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XStateTestPlan {
    /// プランの名前
    pub name: String,
    /// テストのパス
    pub paths: Vec<XStateTestPath>,
    /// 事前条件
    pub preconditions: Option<String>,
    /// 説明文
    pub description: Option<String>,
}

/// テストパスを表現する構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XStateTestPath {
    /// パスの名前
    pub name: String,
    /// パスのセグメント
    pub segments: Vec<XStatePathSegment>,
    /// パスの説明
    pub description: Option<String>,
}

/// パスのセグメントを表現する構造体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XStatePathSegment {
    /// 状態名
    pub state: String,
    /// イベント
    pub event: Option<String>,
    /// アサーション（オプション）
    pub assertions: Option<Vec<String>>,
}

impl<S, E> XStateTestModel<S, E>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static,
{
    /// 新しいテストモデルを作成
    pub fn new(machine: Machine<S, E>) -> Self {
        Self {
            machine,
            implementations: HashMap::new(),
            assertions: HashMap::new(),
            verified_paths: HashSet::new(),
        }
    }

    /// アクターの実装を提供
    pub fn provide<F>(&mut self, actor_id: &str, implementation: F) -> &mut Self
    where
        F: Fn(&mut Context, &Event) -> Result<()> + 'static,
    {
        self.implementations
            .insert(actor_id.to_string(), Box::new(implementation));
        self
    }

    /// 状態に対するアサーションを追加
    pub fn assert<F>(&mut self, assertion_id: &str, assertion: F) -> &mut Self
    where
        F: Fn(&Machine<S, E>) -> bool + 'static,
    {
        self.assertions
            .insert(assertion_id.to_string(), Box::new(assertion));
        self
    }

    /// テストプランの実行
    pub fn test_plan(&mut self, plan: &XStateTestPlan) -> Result<TestPlanResult> {
        let mut results = Vec::new();

        for path in &plan.paths {
            let result = self.test_path(path)?;
            results.push(result);
        }

        Ok(TestPlanResult {
            plan_name: plan.name.clone(),
            path_results: results,
            success: results.iter().all(|r| r.success),
            all_paths_covered: results.len() == plan.paths.len(),
        })
    }

    /// 単一のパスをテスト
    pub fn test_path(&mut self, path: &XStateTestPath) -> Result<TestPathResult> {
        // 実行前にマシンをクローン
        let mut machine_clone = self.machine.clone();
        let mut failures = Vec::new();
        let mut success = true;

        // パスをハッシュ化してパス識別子として使用
        let path_hash = format!("{:?}", path);
        self.verified_paths.insert(path_hash);

        for (i, segment) in path.segments.iter().enumerate() {
            // 現在の状態を確認
            if !machine_clone.is_in(&segment.state) {
                success = false;
                failures.push(SegmentFailure {
                    segment_index: i,
                    state: segment.state.clone(),
                    error: format!("Expected state '{}' but machine is in '{}'", 
                        segment.state, 
                        machine_clone.current_states.iter().next().unwrap_or(&"unknown".to_string())),
                });
                break;
            }

            // アサーションがあれば実行
            if let Some(assertions) = &segment.assertions {
                for assertion_id in assertions {
                    if let Some(assertion) = self.assertions.get(assertion_id) {
                        if !assertion(&machine_clone) {
                            success = false;
                            failures.push(SegmentFailure {
                                segment_index: i,
                                state: segment.state.clone(),
                                error: format!("Assertion '{}' failed", assertion_id),
                            });
                            break;
                        }
                    }
                }
            }

            // エラーが発生していたら中断
            if !success {
                break;
            }

            // 次のイベントを送信
            if let Some(event_type) = &segment.event {
                let event = Event::new(event_type.clone());
                
                // イベント送信
                if let Err(err) = machine_clone.send(event) {
                    success = false;
                    failures.push(SegmentFailure {
                        segment_index: i,
                        state: segment.state.clone(),
                        error: format!("Failed to send event '{}': {}", event_type, err),
                    });
                    break;
                }
            }
        }

        Ok(TestPathResult {
            path_name: path.name.clone(),
            success,
            failures,
            segments_count: path.segments.len(),
        })
    }

    /// ステートマシン全体から可能なパスを生成
    pub fn generate_paths(&self, max_depth: usize) -> XStateTestPlan {
        let mut paths = Vec::new();
        let initial_state = self.machine.initial.clone();
        
        // DFSで可能なパスをすべて探索
        let mut visited = HashSet::new();
        let mut current_path = Vec::new();
        
        self.dfs_generate_paths(
            &initial_state, 
            &mut visited, 
            &mut current_path, 
            &mut paths, 
            0, 
            max_depth
        );
        
        XStateTestPlan {
            name: format!("Generated plan for {}", self.machine.name),
            paths,
            preconditions: None,
            description: Some("Automatically generated test plan".to_string()),
        }
    }
    
    /// DFSを使用して可能なパスを生成
    fn dfs_generate_paths(
        &self,
        current_state: &str,
        visited: &mut HashSet<String>,
        current_path: &mut Vec<XStatePathSegment>,
        paths: &mut Vec<XStateTestPath>,
        depth: usize,
        max_depth: usize,
    ) {
        // 最大深度に達した場合は処理を終了
        if depth >= max_depth {
            // 現在のパスを記録
            if !current_path.is_empty() {
                paths.push(XStateTestPath {
                    name: format!("Path {}", paths.len() + 1),
                    segments: current_path.clone(),
                    description: None,
                });
            }
            return;
        }
        
        // 現在の状態をパスに追加
        current_path.push(XStatePathSegment {
            state: current_state.to_string(),
            event: None,
            assertions: None,
        });
        
        // この状態から可能な遷移をすべて探索
        let outgoing_transitions = self.machine.transitions.iter()
            .filter(|t| t.source == current_state && t.target.is_some())
            .collect::<Vec<_>>();
            
        if outgoing_transitions.is_empty() {
            // 出口がない状態（最終状態）の場合、パスを記録
            paths.push(XStateTestPath {
                name: format!("Path {}", paths.len() + 1),
                segments: current_path.clone(),
                description: None,
            });
        } else {
            // 各遷移を探索
            for transition in outgoing_transitions {
                let target = transition.target.as_ref().unwrap();
                
                // 遷移情報を現在のパスに追加
                current_path.push(XStatePathSegment {
                    state: current_state.to_string(),
                    event: Some(transition.event.clone()),
                    assertions: None,
                });
                
                // 訪問済みでなければ再帰的に探索
                if !visited.contains(&format!("{}-{}", current_state, target)) {
                    visited.insert(format!("{}-{}", current_state, target));
                    self.dfs_generate_paths(target, visited, current_path, paths, depth + 1, max_depth);
                    visited.remove(&format!("{}-{}", current_state, target));
                }
                
                // バックトラック
                current_path.pop();
            }
        }
        
        // バックトラック
        current_path.pop();
    }
    
    /// カバレッジレポートを取得
    pub fn get_coverage(&self) -> TestCoverageReport {
        let total_states = self.machine.states.len();
        let total_transitions = self.machine.transitions.len();
        
        // 探索済みのパスからカバレッジを計算
        let mut visited_states = HashSet::new();
        let mut visited_transitions = HashSet::new();
        
        for path_hash in &self.verified_paths {
            // ここではシンプルに実装
            // 実際には検証済みパスから状態と遷移をカウント
        }
        
        TestCoverageReport {
            visited_states_count: visited_states.len(),
            total_states,
            visited_transitions_count: visited_transitions.len(),
            total_transitions,
        }
    }
}

/// テストプラン実行結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPlanResult {
    /// プラン名
    pub plan_name: String,
    /// 各パスの結果
    pub path_results: Vec<TestPathResult>,
    /// すべてのパスが成功したか
    pub success: bool,
    /// すべてのパスがカバーされたか
    pub all_paths_covered: bool,
}

/// テストパス実行結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPathResult {
    /// パス名
    pub path_name: String,
    /// 成功したか
    pub success: bool,
    /// 失敗情報
    pub failures: Vec<SegmentFailure>,
    /// セグメント数
    pub segments_count: usize,
}

/// セグメント失敗情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentFailure {
    /// セグメントのインデックス
    pub segment_index: usize,
    /// 対象状態
    pub state: String,
    /// エラーメッセージ
    pub error: String,
}

/// テストカバレッジレポート
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCoverageReport {
    /// 訪問した状態の数
    pub visited_states_count: usize,
    /// 総状態数
    pub total_states: usize,
    /// 訪問した遷移の数
    pub visited_transitions_count: usize,
    /// 総遷移数
    pub total_transitions: usize,
}

impl TestCoverageReport {
    /// 状態カバレッジの割合（%）を計算
    pub fn state_coverage_percentage(&self) -> f64 {
        if self.total_states == 0 {
            return 0.0;
        }
        (self.visited_states_count as f64 / self.total_states as f64) * 100.0
    }

    /// 遷移カバレッジの割合（%）を計算
    pub fn transition_coverage_percentage(&self) -> f64 {
        if self.total_transitions == 0 {
            return 0.0;
        }
        (self.visited_transitions_count as f64 / self.total_transitions as f64) * 100.0
    }
}

/// テストモデルを作成するユーティリティ関数
pub fn create_test_model<S, E>(machine: Machine<S, E>) -> XStateTestModel<S, E>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static,
{
    XStateTestModel::new(machine)
}

/// テストプランを実行するユーティリティ関数
pub fn execute_test_plan<S, E>(
    model: &mut XStateTestModel<S, E>,
    plan: &XStateTestPlan,
) -> Result<TestPlanResult>
where
    S: StateTrait + Clone + Debug + Default + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + 'static,
{
    model.test_plan(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, ActionType, MachineBuilder, Transition};

    fn create_test_machine() -> Machine {
        // 状態定義
        let green = crate::State::new("green");
        let yellow = crate::State::new("yellow");
        let red = crate::State::new("red");

        // カウンターをインクリメントするアクション
        let increment_action = Action::new("incrementCounter", ActionType::Transition, |ctx, _evt| {
            let counter = ctx.get::<i32>("counter").unwrap_or(0);
            let _ = ctx.set("counter", counter + 1);
        });

        // 遷移を定義
        let green_to_yellow = Transition::new("green", "TIMER", "yellow");
        let yellow_to_red = Transition::new("yellow", "TIMER", "red");
        let red_to_green = Transition::new("red", "TIMER", "green");

        // マシンを構築
        MachineBuilder::new("trafficLight")
            .state(green)
            .state(yellow)
            .state(red)
            .initial("green")
            .transition(green_to_yellow)
            .transition(yellow_to_red)
            .transition(red_to_green)
            .with_action(increment_action)
            .build()
            .unwrap()
    }

    #[test]
    fn test_create_model() {
        let machine = create_test_machine();
        let model = create_test_model(machine);
        assert_eq!(model.machine.name, "trafficLight");
    }

    #[test]
    fn test_generate_paths() {
        let machine = create_test_machine();
        let model = create_test_model(machine);
        
        let plan = model.generate_paths(3);
        assert!(!plan.paths.is_empty());
    }

    #[test]
    fn test_execute_plan() {
        let machine = create_test_machine();
        let mut model = create_test_model(machine);
        
        // カウンターが増加するという条件を検証するアサーション
        model.assert("counterIncreased", |m| {
            let counter = m.context.get::<i32>("counter").unwrap_or(0);
            counter > 0
        });
        
        // テストプランを作成
        let plan = XStateTestPlan {
            name: "Traffic Light Test".to_string(),
            paths: vec![
                XStateTestPath {
                    name: "Full Cycle".to_string(),
                    segments: vec![
                        XStatePathSegment {
                            state: "green".to_string(),
                            event: None,
                            assertions: None,
                        },
                        XStatePathSegment {
                            state: "green".to_string(),
                            event: Some("TIMER".to_string()),
                            assertions: None,
                        },
                        XStatePathSegment {
                            state: "yellow".to_string(),
                            event: Some("TIMER".to_string()),
                            assertions: None,
                        },
                        XStatePathSegment {
                            state: "red".to_string(),
                            event: Some("TIMER".to_string()),
                            assertions: Some(vec!["counterIncreased".to_string()]),
                        },
                        XStatePathSegment {
                            state: "green".to_string(),
                            event: None,
                            assertions: None,
                        },
                    ],
                    description: Some("Test a full cycle through all states".to_string()),
                },
            ],
            preconditions: None,
            description: Some("Test the traffic light state machine".to_string()),
        };
        
        let result = execute_test_plan(&mut model, &plan);
        assert!(result.is_ok());
        
        let result = result.unwrap();
        assert!(result.success);
        assert_eq!(result.path_results.len(), 1);
        assert!(result.path_results[0].success);
    }
} 