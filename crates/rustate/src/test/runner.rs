use crate::{Machine, Result, Error};
use super::generator::TestCase;
use std::collections::HashSet;
use serde::{Serialize, Deserialize};

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
}

/// テストを実行するランナー
#[derive(Debug)]
pub struct TestRunner<'a> {
    /// 参照する状態マシン
    machine: &'a Machine,
    /// テスト中に訪問した状態
    visited_states: HashSet<String>,
    /// テスト中に訪問した遷移
    visited_transitions: HashSet<String>,
}

impl<'a> TestRunner<'a> {
    /// 新しいテストランナーを作成
    pub fn new(machine: &'a Machine) -> Self {
        Self {
            machine,
            visited_states: HashSet::new(),
            visited_transitions: HashSet::new(),
        }
    }
    
    /// テストケースを実行
    pub fn run_test(&mut self, test_case: &TestCase) -> TestResult {
        // マシンのコピーを作成して変更を追跡
        let mut machine_clone = self.machine.clone();
        
        // 必要に応じてマシンを初期状態に設定
        if test_case.initial_state != self.machine.initial {
            // 適切な初期状態を持つ新しいマシンを作成することは難しいので、
            // ここではシンプルに初期状態からテストケースの初期状態まで
            // 移動するために必要なイベントを送る（実際のケースでは複雑になる可能性がある）
            match self.initialize_to_state(&mut machine_clone, &test_case.initial_state) {
                Ok(_) => {},
                Err(err) => {
                    return TestResult {
                        test_name: test_case.name.clone(),
                        success: false,
                        actual_state: machine_clone.current_states.iter().next()
                            .unwrap_or(&"unknown".to_string()).clone(),
                        expected_state: test_case.initial_state.clone(),
                        error_message: Some(format!("Failed to initialize to state: {}", err)),
                    };
                }
            }
        }
        
        // テスト中の状態追跡を開始
        self.visited_states.insert(machine_clone.current_states.iter().next()
            .unwrap_or(&"unknown".to_string()).clone());
        
        // イベントを順番に送信
        for event in &test_case.events {
            // 遷移を記録
            let current_state = machine_clone.current_states.iter().next()
                .unwrap_or(&"unknown".to_string()).clone();
            
            // イベント送信
            match machine_clone.send(event.clone()) {
                Ok(_) => {},
                Err(err) => {
                    return TestResult {
                        test_name: test_case.name.clone(),
                        success: false,
                        actual_state: current_state,
                        expected_state: test_case.expected_state.clone(),
                        error_message: Some(format!("Error sending event: {}", err)),
                    };
                }
            }
            
            // 新しい状態を記録
            let new_state = machine_clone.current_states.iter().next()
                .unwrap_or(&"unknown".to_string()).clone();
            
            // 遷移を記録
            self.visited_states.insert(new_state.clone());
            self.visited_transitions.insert(format!("{} --{}--> {}", 
                current_state, event.event_type, new_state));
        }
        
        // 最終状態を確認
        let final_state = machine_clone.current_states.iter().next()
            .unwrap_or(&"unknown".to_string()).clone();
        
        let success = final_state == test_case.expected_state;
        
        TestResult {
            test_name: test_case.name.clone(),
            success,
            actual_state: final_state.clone(),
            expected_state: test_case.expected_state.clone(),
            error_message: if success { 
                None 
            } else { 
                Some(format!("Expected state: {}, but got: {}", 
                    test_case.expected_state, final_state)) 
            },
        }
    }
    
    /// 複数のテストケースを実行
    pub fn run_tests(&mut self, test_cases: Vec<TestCase>) -> TestResults {
        let mut results = Vec::new();
        
        for test_case in test_cases {
            let result = self.run_test(&test_case);
            results.push(result);
        }
        
        // カバレッジレポートを作成
        let coverage = CoverageReport {
            visited_states: self.visited_states.clone(),
            visited_transitions: self.visited_transitions.clone(),
            total_states: self.machine.states.len(),
            total_transitions: self.machine.transitions.len(),
        };
        
        TestResults {
            results,
            coverage,
        }
    }
    
    /// マシンを特定の状態に初期化する（シンプルな実装）
    fn initialize_to_state(&self, machine: &mut Machine, target_state: &str) -> Result<()> {
        // 最もシンプルなケース: 既に目的の状態にいる
        if machine.is_in(target_state) {
            return Ok(());
        }
        
        // target_stateに到達するためのパスを探す（この実装は簡略化されています）
        Err(Error::InvalidConfiguration(format!(
            "Cannot initialize to state: {}. This is a limitation of the current implementation.",
            target_state
        )))
    }
} 