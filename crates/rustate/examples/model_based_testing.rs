use rustate::{
    Action, ActionType, Machine, MachineBuilder, State, Transition,
    TestGenerator, TestRunner, Property, PropertyType, ModelChecker
};

fn main() {
    // 状態マシンを作成
    let machine = create_test_machine();
    println!("状態マシンを作成しました: {}", machine.name);
    
    // テストケース生成
    println!("\n=== テストケースの生成 ===");
    let mut generator = TestGenerator::new(&machine);
    
    println!("\n>> 全状態カバレッジのテストケース");
    let state_tests = generator.generate_all_states();
    for test in &state_tests {
        println!("- {}: {} -> {} (イベント数: {})", 
            test.name, test.initial_state, test.expected_state, test.events.len());
    }
    
    println!("\n>> 全遷移カバレッジのテストケース");
    let transition_tests = generator.generate_all_transitions();
    for test in &transition_tests {
        println!("- {}: {} -> {} (イベント数: {})", 
            test.name, test.initial_state, test.expected_state, test.events.len());
    }
    
    println!("\n>> ループカバレッジのテストケース");
    let loop_tests = generator.generate_loop_coverage();
    for test in &loop_tests {
        println!("- {}: {} -> {} (イベント数: {})", 
            test.name, test.initial_state, test.expected_state, test.events.len());
    }
    
    // テスト実行
    println!("\n=== テスト実行 ===");
    let mut runner = TestRunner::new(&machine);
    
    println!("\n>> 状態カバレッジテストの実行");
    let state_results = runner.run_tests(state_tests);
    print_test_results(&state_results);
    
    println!("\n>> 遷移カバレッジテストの実行");
    let transition_results = runner.run_tests(transition_tests);
    print_test_results(&transition_results);
    
    // モデル検査
    println!("\n=== モデル検査 ===");
    let mut checker = ModelChecker::new(&machine);
    
    // 到達可能性プロパティ
    let reachability = Property {
        name: "赤信号に到達可能か".to_string(),
        property_type: PropertyType::Reachability,
        target_states: vec!["red".to_string()],
        description: Some("赤信号状態に到達できることを確認".to_string()),
    };
    
    let result = checker.verify_property(&reachability);
    println!("\n>> プロパティ: {}", reachability.name);
    println!("結果: {}", if result.satisfied { "満たされています" } else { "満たされていません" });
    if let Some(msg) = result.message {
        println!("メッセージ: {}", msg);
    }
    
    // 安全性プロパティ
    let safety = Property {
        name: "存在しない状態に到達しないか".to_string(),
        property_type: PropertyType::Safety,
        target_states: vec!["invalid".to_string()],
        description: Some("存在しない状態に決して到達しないことを確認".to_string()),
    };
    
    let result = checker.verify_property(&safety);
    println!("\n>> プロパティ: {}", safety.name);
    println!("結果: {}", if result.satisfied { "満たされています" } else { "満たされていません" });
    if let Some(msg) = result.message {
        println!("メッセージ: {}", msg);
    }
    
    // デッドロックの検出
    println!("\n>> デッドロック検出");
    let deadlocks = checker.detect_deadlocks();
    if deadlocks.is_empty() {
        println!("デッドロック状態はありません");
    } else {
        println!("デッドロック状態: {}", deadlocks.join(", "));
    }
    
    // 到達不可能な状態の検出
    println!("\n>> 到達不可能な状態の検出");
    let unreachable = checker.detect_unreachable_states();
    if unreachable.is_empty() {
        println!("到達不可能な状態はありません");
    } else {
        println!("到達不可能な状態: {}", unreachable.join(", "));
    }
}

// テスト用の信号機状態マシンを作成
fn create_test_machine() -> Machine {
    // 状態の作成
    let green = State::new("green");
    let yellow = State::new("yellow");
    let red = State::new("red");

    // 遷移の作成
    let green_to_yellow = Transition::new("green", "TIMER", "yellow");
    let yellow_to_red = Transition::new("yellow", "TIMER", "red");
    let red_to_green = Transition::new("red", "TIMER", "green");

    // アクションの定義
    let log_green = Action::new(
        "logGreen",
        ActionType::Entry,
        |_ctx, _evt| println!("緑信号になりました - 進んでください"),
    );
    
    let log_yellow = Action::new(
        "logYellow",
        ActionType::Entry,
        |_ctx, _evt| println!("黄信号になりました - 注意してください"),
    );
    
    let log_red = Action::new(
        "logRed",
        ActionType::Entry,
        |_ctx, _evt| println!("赤信号になりました - 停止してください"),
    );

    // マシンの構築
    MachineBuilder::new("trafficLight")
        .state(green)
        .state(yellow)
        .state(red)
        .initial("green")
        .transition(green_to_yellow)
        .transition(yellow_to_red)
        .transition(red_to_green)
        .on_entry("green", log_green)
        .on_entry("yellow", log_yellow)
        .on_entry("red", log_red)
        .build()
        .unwrap()
}

// テスト結果を出力
fn print_test_results(results: &rustate::TestResults) {
    println!("総テスト数: {}", results.total_count());
    println!("成功: {}", results.success_count());
    println!("失敗: {}", results.failure_count());
    println!("成功率: {:.1}%", results.success_rate());
    
    // カバレッジレポート
    let coverage = results.get_coverage();
    println!("状態カバレッジ: {:.1}%", coverage.state_coverage());
    println!("遷移カバレッジ: {:.1}%", coverage.transition_coverage());
    
    // 失敗したテストがあれば詳細を表示
    let failed_tests = results.results.iter().filter(|r| !r.success).collect::<Vec<_>>();
    if !failed_tests.is_empty() {
        println!("\n失敗したテスト:");
        for failed in failed_tests {
            println!("- {}: 期待={}, 実際={}", 
                failed.test_name, failed.expected_state, failed.actual_state);
            if let Some(err) = &failed.error_message {
                println!("  エラー: {}", err);
            }
        }
    }
} 