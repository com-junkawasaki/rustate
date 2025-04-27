use crate::event::EventTrait;
use crate::{Event, Machine};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

/// テストケースを表現する構造体
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestCase {
    /// テストケースの名前
    pub name: String,
    /// 初期状態
    pub initial_state: String,
    /// 送信するイベントのシーケンス
    pub events: Vec<Event>,
    /// 期待される最終状態
    pub expected_state: String,
}

/// モデルからテストケースを生成するジェネレータ
#[derive(Debug)]
pub struct TestGenerator<'a> {
    /// 参照するマシンモデル
    machine: &'a Machine,
    /// 既に訪問した状態の組み合わせ
    visited_states: HashSet<String>,
}

impl<'a> TestGenerator<'a> {
    /// 新しいテストジェネレータを作成
    pub fn new(machine: &'a Machine) -> Self {
        Self {
            machine,
            visited_states: HashSet::new(),
        }
    }

    /// 全ての状態を訪問するテストケースを生成
    pub fn generate_all_states(&mut self) -> Vec<TestCase> {
        let mut test_cases = Vec::new();
        let mut state_queue: VecDeque<String> = VecDeque::new();

        // 開始状態を追加
        state_queue.push_back(self.machine.initial.clone());
        self.visited_states.clear();

        while let Some(state_id) = state_queue.pop_front() {
            if self.visited_states.contains(&state_id) {
                continue;
            }

            self.visited_states.insert(state_id.clone());

            // この状態に到達するためのテストケースを生成
            if let Some(test_case) = self.generate_test_to_state(&state_id) {
                test_cases.push(test_case);
            }

            // この状態から遷移可能な次の状態を取得
            for transition in self
                .machine
                .transitions
                .get(&*state_id)
                .unwrap_or(&Vec::new())
                .iter()
            {
                if transition.target.is_some() {
                    let target = transition.target.as_ref().unwrap();
                    if !self.visited_states.contains(target) {
                        state_queue.push_back(target.clone());
                    }
                }
            }
        }

        test_cases
    }

    /// 全ての遷移をテストするテストケースを生成
    pub fn generate_all_transitions(&mut self) -> Vec<TestCase> {
        let mut test_cases = Vec::new();

        for transitions_vec in self.machine.transitions.values() {
            for transition in transitions_vec.iter() {
                if let Some(target) = &transition.target {
                    let test_case = TestCase {
                        name: format!(
                            "Transition: {} --{:?}--> {}",
                            transition.source, transition.event, target
                        ),
                        initial_state: transition.source.clone(),
                        events: vec![transition.event.clone().unwrap_or_else(|| Event::new(""))],
                        expected_state: target.clone(),
                    };
                    test_cases.push(test_case);
                }
            }
        }

        test_cases
    }

    /// 指定した状態に到達するテストケースを生成
    fn generate_test_to_state(&self, target_state: &str) -> Option<TestCase> {
        if target_state == self.machine.initial {
            return Some(TestCase {
                name: format!("Initial state: {}", target_state),
                initial_state: target_state.to_string(),
                events: Vec::new(),
                expected_state: target_state.to_string(),
            });
        }

        // 最短パスアルゴリズムでターゲット状態に到達するパスを見つける
        let path = self.find_shortest_path_to_state(target_state)?;

        Some(TestCase {
            name: format!("Path to state: {}", target_state),
            initial_state: self.machine.initial.clone(),
            events: path,
            expected_state: target_state.to_string(),
        })
    }

    /// 最短パスアルゴリズムを使用して、特定の状態に到達するイベントのシーケンスを見つける
    fn find_shortest_path_to_state(&self, target_state: &str) -> Option<Vec<Event>> {
        let mut queue: VecDeque<(String, Vec<Event>)> = VecDeque::new();
        let mut visited = HashSet::new();

        // 開始状態をキューに追加
        queue.push_back((self.machine.initial.clone(), Vec::new()));

        while let Some((current_state, events)) = queue.pop_front() {
            if current_state == target_state {
                return Some(events);
            }

            if visited.contains(&current_state) {
                continue;
            }

            visited.insert(current_state.clone());

            // 現在の状態からすべての遷移を探索
            for transition in self
                .machine
                .transitions
                .get(&*current_state)
                .unwrap_or(&Vec::new())
                .iter()
            {
                if transition.target.is_some() {
                    let next_state = transition.target.as_ref().unwrap();

                    if !visited.contains(next_state) {
                        let mut new_events = events.clone();
                        new_events.push(transition.event.clone().unwrap_or_else(|| Event::new("")));

                        queue.push_back((next_state.clone(), new_events));
                    }
                }
            }
        }

        None
    }

    /// 状態マシンの強連結成分を見つけ、ループのカバレッジを考慮したテストケースを生成
    pub fn generate_loop_coverage(&mut self) -> Vec<TestCase> {
        let mut test_cases = Vec::new();
        let reachable_states = self.find_reachable_states();

        for state in reachable_states {
            // この状態から自分自身に戻るループを検索
            let loops = self.find_loops_from_state(&state);

            for loop_path in loops {
                if !loop_path.is_empty() {
                    let events: Vec<Event> = loop_path
                        .iter()
                        .map(|event_name| Event::new(&event_name))
                        .collect();

                    test_cases.push(TestCase {
                        name: format!("Loop from state: {}", state),
                        initial_state: state.clone(),
                        events,
                        expected_state: state.clone(),
                    });
                }
            }
        }

        test_cases
    }

    /// 状態から自分自身に戻るループを見つける
    fn find_loops_from_state(&self, state: &str) -> Vec<Vec<String>> {
        let mut loops = Vec::new();
        let mut visited = HashSet::new();
        let mut path = Vec::new();

        self.dfs_find_loops(state, state, &mut visited, &mut path, &mut loops);

        loops
    }

    /// DFS(深さ優先探索)を使ってループを見つける
    fn dfs_find_loops(
        &self,
        start: &str,
        current: &str,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
        loops: &mut Vec<Vec<String>>,
    ) {
        if !path.is_empty() && current == start {
            loops.push(path.clone());
            return;
        }

        if visited.contains(current) {
            return;
        }

        visited.insert(current.to_string());

        for transition in self
            .machine
            .transitions
            .get(&*current)
            .unwrap_or(&Vec::new())
            .iter()
        {
            if transition.target.is_some() {
                let target = transition.target.as_ref().unwrap();
                path.push(
                    transition
                        .event
                        .as_ref()
                        .map(|e| e.event_type().to_string())
                        .unwrap_or_default(),
                );

                self.dfs_find_loops(start, target, visited, path, loops);

                path.pop();
            }
        }

        visited.remove(current);
    }

    /// 到達可能な状態をすべて検索
    fn find_reachable_states(&self) -> HashSet<String> {
        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(self.machine.initial.clone());

        while let Some(state) = queue.pop_front() {
            if reachable.contains(&state) {
                continue;
            }

            reachable.insert(state.clone());

            for transition in self
                .machine
                .transitions
                .get(&*state)
                .unwrap_or(&Vec::new())
                .iter()
            {
                if transition.target.is_some() {
                    let target = transition.target.as_ref().unwrap();
                    if !reachable.contains(target) {
                        queue.push_back(target.clone());
                    }
                }
            }
        }

        reachable
    }
}
