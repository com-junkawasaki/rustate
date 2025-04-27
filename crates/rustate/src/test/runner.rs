use super::generator::TestCase;
use crate::{Error, Event, IntoEvent, Machine, Result, StateTrait, Context, Error as StateError};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashSet;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::fmt::{Debug, Display};
use std::hash::Hash;

/// テスト実行結果を表す構造体
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestResult {
    /// テストケースの名前
    pub test_name: String,
    /// テストが成功したかどうか
    pub success: bool,
    /// 実際の最終状態
    pub actual_state: String,
    /// 期待された最終状態
    pub expected_state: String,
    /// エラーメッセージ（失敗時）
    pub error_message: Option<String>,
}

/// カバレッジ情報を表す構造体
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoverageReport {
    /// テストされた状態
    pub visited_states: HashSet<String>,
    /// テストされた遷移
    pub visited_transitions: HashSet<String>,
    /// 全状態の数
    pub total_states: usize,
    /// 全遷移の数
    pub total_transitions: usize,
}

impl CoverageReport {
    /// 状態カバレッジの割合（%）を計算
    pub fn state_coverage(&self) -> f64 {
        if self.total_states == 0 {
            return 0.0;
        }

        (self.visited_states.len() as f64 / self.total_states as f64) * 100.0
    }

    /// 遷移カバレッジの割合（%）を計算
    pub fn transition_coverage(&self) -> f64 {
        if self.total_transitions == 0 {
            return 0.0;
        }

        (self.visited_transitions.len() as f64 / self.total_transitions as f64) * 100.0
    }
}

/// テスト結果の集約
#[derive(Clone, Debug)]
pub struct TestResults {
    /// 個々のテスト結果
    pub results: Vec<TestResult>,
    /// カバレッジ情報
    pub coverage: CoverageReport,
}

impl TestResults {
    /// 結果からカバレッジレポートを取得
    pub fn get_coverage(&self) -> &CoverageReport {
        &self.coverage
    }

    /// 成功したテストの数を取得
    pub fn success_count(&self) -> usize {
        self.results.iter().filter(|r| r.success).count()
    }

    /// 失敗したテストの数を取得
    pub fn failure_count(&self) -> usize {
        self.results.iter().filter(|r| !r.success).count()
    }

    /// 総テスト数を取得
    pub fn total_count(&self) -> usize {
        self.results.len()
    }

    /// 成功率を計算（%）
    pub fn success_rate(&self) -> f64 {
        if self.total_count() == 0 {
            return 0.0;
        }

        (self.success_count() as f64 / self.total_count() as f64) * 100.0
    }

    /// 全てのテストが成功したかどうかを確認
    pub fn all_passed(&self) -> bool {
        self.failure_count() == 0 && self.total_count() > 0
    }
}

/// テストを実行するランナー
#[derive(Debug)]
pub struct TestRunner<'a, S = String, E = Event, C = Context>
where
    S: StateTrait + Clone + Debug + Eq + Hash + Display + Send + Sync + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + Send + Sync + 'static,
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    machine: &'a mut Machine<C, E, S>,
    results: TestResults,
    coverage: HashMap<String, usize>,
    _marker: PhantomData<(S, E, C)>,
}

impl<'a, S, E, C> TestRunner<'a, S, E, C>
where
    S: StateTrait + Clone + Debug + Eq + Hash + Display + Send + Sync + 'static,
    E: EventTrait + Clone + Debug + IntoEvent + Send + Sync + 'static,
    C: Clone + Debug + Default + Send + Sync + 'static + Serialize + DeserializeOwned,
{
    /// 新しいテストランナーを作成
    pub fn new(machine: &'a mut Machine<C, E, S>) -> Self {
        Self {
            machine,
            results: TestResults {
                results: Vec::new(),
                coverage: CoverageReport {
                    visited_states: HashSet::new(),
                    visited_transitions: HashSet::new(),
                    total_states: 0,
                    total_transitions: 0,
                },
            },
            coverage: HashMap::new(),
            _marker: PhantomData,
        }
    }

    /// テストケースを実行
    pub async fn run_test(&mut self, test_case: &TestCase) -> TestResult {
        // マシンのコピーを作成して変更を追跡
        // let mut machine_clone = self.machine.clone();
        // Cloning the machine is problematic with async because the builder is now async.
        // Instead, we should ideally use the provided machine and reset it if possible,
        // or create a new machine instance for each test.
        // For simplicity here, let's assume we can create a new builder from the original definition
        // (This requires the definition to be available or reconstructable)
        // A more practical approach might involve a `reset()` method on Machine or using `MachineBuilder::clone()`
        // Let's try cloning the builder if the original machine instance isn't easily reset/rebuilt.
        // However, the TestRunner only has a reference &'a Machine.
        // ***Simplification for now: We'll use the single machine instance and modify it directly.***
        // ***This means tests are NOT isolated and depend on the order they run!***
        // ***A TODO for proper test isolation.***
        // let mut machine_clone = self.machine.clone(); // Can't easily clone Machine now

        // Use the original machine directly (WARNING: NO ISOLATION)
        // Resetting state might be needed, but let's assume tests handle it for now.

        // TODO: Implement proper state initialization/reset for async machine
        // if test_case.initial_state != self.machine.initial { ... }

        // Record initial state (assuming it's correct)
        let initial_state = self
            .machine
            .current_states
            .iter()
            .next()
            .cloned()
            .unwrap_or_default();
        self.results.coverage.visited_states.insert(initial_state);

        // イベントを順番に送信
        let mut last_state = self
            .machine
            .current_states
            .iter()
            .next()
            .cloned()
            .unwrap_or_default();
        for event_like in &test_case.events {
            // Assuming TestCase events are IntoEvent compatible
            let event = event_like.clone().into_event(); // Clone and convert
            let current_state = last_state.clone(); // State before sending event

            // イベント送信
            // match machine_clone.send(event.clone()) {
            match self.machine.send(event.clone()).await {
                // Use self.machine, await send
                Ok(_) => {}
                Err(err) => {
                    return TestResult {
                        test_name: test_case.name.clone(),
                        success: false,
                        actual_state: current_state,
                        expected_state: test_case.expected_state.clone(),
                        error_message: Some(format!(
                            "Error sending event '{}': {}",
                            event.event_type, err
                        )),
                    };
                }
            }

            // 新しい状態を記録
            let new_state = self
                .machine
                .current_states
                .iter()
                .next()
                .cloned()
                .unwrap_or_default(); // Get the new state

            // 遷移を記録
            self.results.coverage.visited_states.insert(new_state.clone());
            self.results.coverage.visited_transitions.insert(format!(
                "{} --{}--> {}",
                current_state, event.event_type, new_state
            ));
            last_state = new_state; // Update last_state for next iteration
        }

        // 最終状態を確認
        let final_state = last_state;
        // let final_state = self.machine
        //     .current_states
        //     .iter()
        //     .next()
        //     .cloned()
        //     .unwrap_or_default();

        let success = final_state == test_case.expected_state;

        TestResult {
            test_name: test_case.name.clone(),
            success,
            actual_state: final_state.clone(),
            expected_state: test_case.expected_state.clone(),
            error_message: if success {
                None
            } else {
                Some(format!(
                    "Expected state: {}, but got: {}",
                    test_case.expected_state, final_state
                ))
            },
        }
    }

    /// 複数のテストケースを実行
    pub async fn run_tests(&mut self, test_cases: Vec<TestCase>) -> TestResults {
        let mut results = Vec::new();

        for test_case in test_cases {
            let result = self.run_test(&test_case).await;
            results.push(result);
        }

        // カバレッジレポートを作成
        let coverage = CoverageReport {
            visited_states: self.results.coverage.visited_states.clone(),
            visited_transitions: self.results.coverage.visited_transitions.clone(),
            total_states: 0, // Placeholder
            total_transitions: self.results.coverage.visited_transitions.len(),
        };

        self.results.coverage = coverage;
        self.results.results = results;
        self.results
    }

    /// マシンを特定の状態に初期化する（シンプルな実装）
    fn initialize_to_state(&self, _machine: &mut Machine<C, E, S>, _target_state: &str) -> Result<()> {
        // TODO: Implement async state initialization if needed
        // This likely involves finding a path and sending events with await.
        Ok(())
    }

    pub fn generate_report(&self) -> CoverageReport {
        let visited_states_count = self.coverage.len();
        // FIXME: Find correct way to get total state count
        // let total_states = self.machine.states.len();
        let total_states = 0; // Placeholder
        let coverage_percentage = if total_states > 0 {
            (visited_states_count as f64 / total_states as f64) * 100.0
        } else {
            0.0
        };

        CoverageReport {
            visited_states: self.coverage.keys().cloned().collect(),
            visited_transitions: self.results.coverage.visited_transitions.clone(),
            total_states, // Use the placeholder
            total_transitions: self.results.coverage.visited_transitions.len(),
        }
    }
}
