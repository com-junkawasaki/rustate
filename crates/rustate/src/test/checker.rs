use crate::{Event, Machine, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// 検証プロパティの種類
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PropertyType {
    /// 到達可能性: 特定の状態に到達可能かどうか
    Reachability,
    /// 安全性: 特定の状態に「決して」到達しないか
    Safety,
    /// 活性: いつかは特定の状態に到達するか
    Liveness,
    /// 公平性: 特定の状態に何度でも到達可能か
    Fairness,
}

/// 検証プロパティを表す構造体
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Property {
    /// プロパティの名前
    pub name: String,
    /// プロパティの種類
    pub property_type: PropertyType,
    /// プロパティに関連する状態
    pub target_states: Vec<String>,
    /// 記述（オプション）
    pub description: Option<String>,
}

/// 検証結果
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationResult {
    /// プロパティの名前
    pub property_name: String,
    /// 検証が成功したかどうか
    pub satisfied: bool,
    /// 反例（プロパティ違反が見つかった場合）
    pub counterexample: Option<Vec<Event>>,
    /// 追加情報
    pub message: Option<String>,
}

/// モデルチェッカー
#[derive(Debug)]
pub struct ModelChecker<'a> {
    /// 参照するマシン
    machine: &'a Machine,
    /// 既に探索した状態のセット
    visited_states: HashSet<String>,
    /// 状態から状態への遷移を記録するマップ
    transition_graph: HashMap<String, Vec<(String, String)>>,
}

impl<'a> ModelChecker<'a> {
    /// 新しいモデルチェッカーを作成
    pub fn new(machine: &'a Machine) -> Self {
        Self {
            machine,
            visited_states: HashSet::new(),
            transition_graph: HashMap::new(),
        }
    }

    /// 状態マシンのフルグラフを構築
    fn build_graph(&mut self) -> Result<()> {
        self.visited_states.clear();
        self.transition_graph.clear();

        let mut queue = VecDeque::new();
        queue.push_back(self.machine.initial.clone());

        while let Some(state) = queue.pop_front() {
            if self.visited_states.contains(&state) {
                continue;
            }

            self.visited_states.insert(state.clone());

            // この状態からの全遷移を見つける
            let outgoing_transitions = self
                .machine
                .transitions
                .get(&state)
                .unwrap_or(&Vec::new())
                .iter()
                .filter(|t| t.target.is_some())
                .map(|t| {
                    // Convert Option<Event> to String using event_type
                    let event_str = t
                        .event
                        .as_ref()
                        .map_or_else(|| "".to_string(), |e| e.event_type().to_string());
                    (event_str, t.target.clone().unwrap())
                })
                .collect::<Vec<_>>();

            self.transition_graph
                .insert(state.clone(), outgoing_transitions.clone());

            // 未訪問の次の状態をキューに追加
            for (_, target) in outgoing_transitions {
                if !self.visited_states.contains(&target) {
                    queue.push_back(target);
                }
            }
        }

        Ok(())
    }

    /// プロパティを検証
    pub fn verify_property(&mut self, property: &Property) -> VerificationResult {
        // グラフの構築
        if let Err(err) = self.build_graph() {
            return VerificationResult {
                property_name: property.name.clone(),
                satisfied: false,
                counterexample: None,
                message: Some(format!("Error building state graph: {}", err)),
            };
        }

        match property.property_type {
            PropertyType::Reachability => self.verify_reachability(property),
            PropertyType::Safety => self.verify_safety(property),
            PropertyType::Liveness => self.verify_liveness(property),
            PropertyType::Fairness => self.verify_fairness(property),
        }
    }

    /// 到達可能性プロパティを検証
    fn verify_reachability(&self, property: &Property) -> VerificationResult {
        let mut result = VerificationResult {
            property_name: property.name.clone(),
            satisfied: false,
            counterexample: None,
            message: None,
        };

        // すべてのターゲット状態が到達可能か確認
        for target in &property.target_states {
            if !self.visited_states.contains(target) {
                result.message = Some(format!("State '{}' is not reachable", target));
                return result;
            }
        }

        // 指定したすべての状態に到達可能
        result.satisfied = true;
        result.message = Some("All target states are reachable".to_string());

        result
    }

    /// 安全性プロパティを検証
    fn verify_safety(&self, property: &Property) -> VerificationResult {
        let mut result = VerificationResult {
            property_name: property.name.clone(),
            satisfied: true,
            counterexample: None,
            message: None,
        };

        // いずれかのターゲット状態が到達可能であれば安全性違反
        for target in &property.target_states {
            if self.visited_states.contains(target) {
                result.satisfied = false;

                // 初期状態からターゲット状態への経路を見つける
                if let Some(path) = self.find_path_to_state(target) {
                    result.counterexample = Some(path);
                }

                result.message = Some(format!("Safety violation: State '{}' is reachable", target));
                return result;
            }
        }

        // 指定したすべての状態に到達不可能
        result.message = Some("None of the target states are reachable".to_string());

        result
    }

    /// 活性プロパティを検証（簡易実装）
    fn verify_liveness(&self, property: &Property) -> VerificationResult {
        // 活性プロパティの完全な検証には時間的モデル検査が必要
        // この実装は簡略化されています
        let mut result = VerificationResult {
            property_name: property.name.clone(),
            satisfied: false,
            counterexample: None,
            message: None,
        };

        // この簡易実装では到達可能性と同じチェックを行います
        for target in &property.target_states {
            if !self.visited_states.contains(target) {
                result.message = Some(format!(
                    "Liveness violation: State '{}' is not reachable",
                    target
                ));
                return result;
            }
        }

        // TODO: デッドロック検出やループ検出の実装

        result.satisfied = true;
        result.message =
            Some("Liveness property seems to be satisfied (simplified check)".to_string());

        result
    }

    /// 公平性プロパティを検証（簡易実装）
    fn verify_fairness(&self, property: &Property) -> VerificationResult {
        // 公平性の完全な検証にはより高度なアルゴリズムが必要
        // この実装は簡略化されています
        let mut result = VerificationResult {
            property_name: property.name.clone(),
            satisfied: false,
            counterexample: None,
            message: None,
        };

        // すべてのターゲット状態が強連結成分内にあるか確認
        // （単純化した実装では循環パスの存在をチェック）
        for target in &property.target_states {
            if !self.visited_states.contains(target) {
                result.message = Some(format!("State '{}' is not reachable", target));
                return result;
            }

            // ターゲットから自分自身への循環パスが存在するか確認
            if !self.has_cycle_through_state(target) {
                result.message = Some(format!(
                    "Fairness violation: No infinite path through state '{}'",
                    target
                ));
                return result;
            }
        }

        result.satisfied = true;
        result.message =
            Some("Fairness property seems to be satisfied (simplified check)".to_string());

        result
    }

    /// 指定した状態を通る循環パスが存在するか確認
    fn has_cycle_through_state(&self, state: &str) -> bool {
        if let Some(transitions) = self.transition_graph.get(state) {
            for (_, target) in transitions {
                // DFSで循環パスを探索
                let mut visited = HashSet::new();
                if self.dfs_find_path(target, state, &mut visited) {
                    return true;
                }
            }
        }

        false
    }

    /// DFSで2点間のパスを探索
    fn dfs_find_path(&self, current: &str, target: &str, visited: &mut HashSet<String>) -> bool {
        if current == target {
            return true;
        }

        if visited.contains(current) {
            return false;
        }

        visited.insert(current.to_string());

        if let Some(transitions) = self.transition_graph.get(current) {
            for (_, next) in transitions {
                if self.dfs_find_path(next, target, visited) {
                    return true;
                }
            }
        }

        false
    }

    /// 初期状態から特定の状態へのパス（イベントのシーケンス）を見つける
    fn find_path_to_state(&self, target: &str) -> Option<Vec<Event>> {
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut path_map: HashMap<String, (String, String)> = HashMap::new();

        queue.push_back(self.machine.initial.clone());
        visited.insert(self.machine.initial.clone());

        while let Some(current) = queue.pop_front() {
            if current == target {
                // パスを再構築
                let mut path = Vec::new();
                let mut current_state = current.clone();

                while current_state != self.machine.initial {
                    let (prev_state, event) = path_map.get(&current_state).unwrap().clone();
                    path.push(Event::new(&event));
                    current_state = prev_state;
                }

                path.reverse();
                return Some(path);
            }

            if let Some(transitions) = self.transition_graph.get(&current) {
                for (event, next) in transitions {
                    if !visited.contains(next) {
                        visited.insert(next.clone());
                        path_map.insert(next.clone(), (current.clone(), event.clone()));
                        queue.push_back(next.clone());
                    }
                }
            }
        }

        None
    }

    /// すべてのリーチャブルな状態を取得
    pub fn get_reachable_states(&mut self) -> HashSet<String> {
        if self.visited_states.is_empty() {
            let _ = self.build_graph();
        }

        self.visited_states.clone()
    }

    /// デッドロック状態を検出
    pub fn detect_deadlocks(&mut self) -> Vec<String> {
        if self.visited_states.is_empty() {
            let _ = self.build_graph();
        }

        let mut deadlocks = Vec::new();

        for state in &self.visited_states {
            if let Some(transitions) = self.transition_graph.get(state) {
                if transitions.is_empty() {
                    deadlocks.push(state.clone());
                }
            }
        }

        deadlocks
    }

    /// 到達不可能な状態を検出
    pub fn detect_unreachable_states(&mut self) -> Vec<String> {
        if self.visited_states.is_empty() {
            let _ = self.build_graph();
        }

        let all_states: HashSet<_> = self.machine.states.all().map(|s| s.id.clone()).collect();
        let unreachable: Vec<_> = all_states
            .difference(&self.visited_states)
            .cloned()
            .collect();

        unreachable
    }
}
