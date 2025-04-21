use rustate::{
    Action, ActionType, Context, Event, EventSequenceStrategyBuilder, Machine, MachineBuilder,
    PropertyTestRunner, State, Transition,
};
use proptest::test_runner::Config;

fn main() {
    // 信号機の状態マシンを作成
    let machine = create_traffic_light_machine();
    
    println!("=== Property-Based Testing Example ===");
    
    // 単純なプロパティテスト
    let property1 = Machine::property("green to yellow transition")
        .description("When in green state, sending TIMER should transition to yellow")
        .given(|m| m.is_in("green"))
        .when(|m| m.send("TIMER"))
        .then(|m| m.is_in("yellow"));
    
    let runner = PropertyTestRunner::new(machine.clone());
    let result1 = runner.verify_property(property1, Config::default());
    
    println!("Property 1: {}", result1.property_name);
    println!("Success: {}", result1.success);
    println!("Message: {}", result1.message.unwrap_or_default());
    println!();
    
    // イベントシーケンスを使ったプロパティテスト
    let property2 = Machine::property("cycle property")
        .description("Sending TIMER three times from any state should complete a full cycle")
        .given(|_| true) // どの状態からでも
        .when(|m| {
            let initial_state = m.current_state().id().to_string();
            m.send("TIMER")?;
            m.send("TIMER")?;
            m.send("TIMER")?;
            Ok(m.current_state().clone())
        })
        .then(|m| {
            let current = m.current_state().id();
            let initial = m.initial.as_str();
            println!("Current: {}, Initial: {}", current, initial);
            current == initial
        });
    
    // イベントシーケンスストラテジーの構築
    let events_strategy = EventSequenceStrategyBuilder::<_, Event>::new()
        .with_events(vec![Event::new("TIMER")])
        .min_length(1)
        .max_length(5)
        .build();
    
    let result2 = runner.verify_with_events(property2, events_strategy, Config::default());
    
    println!("Property 2: {}", result2.property_name);
    println!("Success: {}", result2.success);
    println!("Message: {}", result2.message.unwrap_or_default());
    
    // カスタムプロパティ: 不変条件
    let invariant_property = Machine::property("color sequence invariant")
        .description("Traffic light must always follow the correct sequence")
        .given(|_| true)
        .when(|m| {
            // ランダムにイベントを適用
            for _ in 0..10 {
                let _ = m.send("TIMER");
            }
            Ok(m.current_state().clone())
        })
        .then(|m| {
            // 状態遷移の履歴を取得
            let history = m.history();
            
            // 正しい順序で遷移しているか確認
            for window in history.windows(2) {
                if let [prev, next] = window {
                    // 許可されている遷移のみ
                    match (prev.as_str(), next.as_str()) {
                        ("green", "yellow") => {}
                        ("yellow", "red") => {}
                        ("red", "green") => {}
                        _ => return false, // 不正な遷移
                    }
                }
            }
            true
        });
    
    let result3 = runner.verify_property(invariant_property, Config::default());
    
    println!("\nProperty 3: {}", result3.property_name);
    println!("Success: {}", result3.success);
    println!("Message: {}", result3.message.unwrap_or_default());
}

// 信号機の状態マシンを作成する関数
fn create_traffic_light_machine() -> Machine {
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
        |_ctx: &mut Context, _evt: &Event| println!("Entering GREEN state - Go!"),
    );
    
    let log_yellow = Action::new(
        "logYellow",
        ActionType::Entry,
        |_ctx: &mut Context, _evt: &Event| println!("Entering YELLOW state - Prepare to stop!"),
    );
    
    let log_red = Action::new(
        "logRed",
        ActionType::Entry,
        |_ctx: &mut Context, _evt: &Event| println!("Entering RED state - Stop!"),
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