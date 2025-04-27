use super::generator::TestCase;
use crate::{
    Context, Error, Error as StateError, Event, EventTrait, IntoEvent, Machine, Result, StateTrait,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::marker::PhantomData;

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
pub struct TestRunner<S, E, C>
where
    S: StateTrait
        + Clone
        + Debug
        + Eq
        + Hash
        + Display
        + From<String>
        + Default
        + Serialize
        + DeserializeOwned,
    E: EventTrait + Clone + Debug + IntoEvent + Serialize + DeserializeOwned,
    C: Default + Clone + Debug,
{
    machine: Machine<C, E, S>,
    test_cases: Vec<TestCase>,
    results: HashMap<String, Result<(), Error>>,
}

impl<S, E, C> TestRunner<S, E, C>
where
    S: StateTrait
        + Clone
        + Debug
        + Eq
        + Hash
        + Display
        + From<String>
        + Default
        + Serialize
        + DeserializeOwned,
    E: EventTrait + Clone + Debug + IntoEvent + Serialize + DeserializeOwned,
    C: Default + Clone + Debug,
{
    pub fn new(machine: Machine<C, E, S>) -> Self {
        Self {
            machine,
            test_cases: Vec::new(),
            results: HashMap::new(),
        }
    }

    pub fn add_test_case(&mut self, test_case: TestCase) {
        self.test_cases.push(test_case);
    }

    pub async fn run_all_tests(&mut self) {
        for test_case in &self.test_cases {
            let result = self.run_test_case(test_case).await;
            self.results.insert(test_case.name.clone(), result);
        }
    }

    async fn run_test_case(&self, test_case: &TestCase) -> Result<(), Error> {
        let mut machine = self.machine.clone();
        machine.reset(); // Ensure machine starts from initial state
        machine.set_context(test_case.initial_context.clone().unwrap_or_default());

        let mut visited_states: HashSet<String> = HashSet::new();
        visited_states.insert(machine.current_state().to_string()); // Convert S to String

        for event_input in &test_case.events {
            let event = event_input.clone().into_event()?;
            let result = machine.send(event).await;

            if let Err(e) = result {
                return Err(e.into());
            }
            visited_states.insert(machine.current_state().to_string()); // Convert S to String
        }

        let final_state = machine.current_state();
        if final_state.to_string() != test_case.expected_state.to_string() {
            // Convert S to String for comparison
            return Err(Error::AssertionFailed(format!(
                "Test case '{}': Expected final state '{}', but got '{}'",
                test_case.name,
                test_case.expected_state.to_string(), // Convert S to String
                final_state.to_string()               // Convert S to String
            )));
        }

        // Optional: Check expected context if provided
        if let Some(expected_context) = &test_case.expected_context {
            if &machine.get_context() != expected_context {
                return Err(Error::AssertionFailed(format!(
                    "Test case '{}': Expected final context {:?}, but got {:?}",
                    test_case.name,
                    expected_context,
                    machine.get_context()
                )));
            }
        }

        Ok(())
    }

    pub fn get_results(&self) -> HashMap<String, Result<(), Error>> {
        self.results.clone() // Clone results to avoid move error
    }
}
