use rustate::{
    Action, ActionType, Context, Event, Machine, MachineBuilder,
    PropertyTestRunner, State, Transition,
};
use proptest::test_runner::Config;

fn main() {
    // 信号機の状態マシンを作成（マッパー付き）
    let machine = create_traffic_light_machine().with_state_mapper(|id| {
        // IDに基づいてStateオブジェクトを返す
        State::new(id)
    });
    
    println!("=== Property-Based Testing Example ===");
    
    // 単純なプロパティテスト - 明示的に型を指定
    let property1 = Machine::<State, Event>::property("green to yellow transition")
        .description("When in green state, sending TIMER should transition to yellow")
        .given(|m| m.is_in("green"))
        .when(|m| {
            // sendはboolを返すため、適切な戻り値型にする
            let _ = m.send("TIMER");
            Ok(m.current_state().clone())
        })
        .then(|m| m.is_in("yellow"));
    
    let runner = PropertyTestRunner::new(machine.clone());
    let result1 = runner.verify_property(property1, Config::default());
    
    println!("Property 1: {}", result1.property_name);
    println!("Success: {}", result1.success);
    println!("Message: {}", result1.message.unwrap_or_default());
    println!();
    
    // イベントシーケンスを使ったプロパティテスト - 明示的に型を指定
    let property2 = Machine::<State, Event>::property("cycle property")
        .description("Sending TIMER three times from any state should complete a full cycle")
        .given(|_| true) // どの状態でも
        .when(|m| {
            // 3回のイベント送信
            let _ = m.send("TIMER");
            let _ = m.send("TIMER");
            let _ = m.send("TIMER");
            Ok(m.current_state().clone())
        })
        .then(|m| {
            // 元の状態に戻っているか確認
            m.is_in("green")
        });
    
    let result2 = runner.verify_property(property2, Config::default());
    
    println!("Property 2: {}", result2.property_name);
    println!("Success: {}", result2.success);
    println!("Message: {}", result2.message.unwrap_or_default());
    
    // カスタムプロパティ: 不変条件 - 明示的に型を指定
    let invariant_property = Machine::<State, Event>::property("traffic light sequence")
        .description("Traffic light follows the correct sequence")
        .given(|_| true)
        .when(|m| {
            // 順序の正しさをチェックする簡単な方法
            let _ = m.send("TIMER"); // green -> yellow
            if !m.is_in("yellow") {
                return Ok(m.current_state().clone()); // 失敗
            }
            
            let _ = m.send("TIMER"); // yellow -> red
            if !m.is_in("red") {
                return Ok(m.current_state().clone()); // 失敗
            }
            
            let _ = m.send("TIMER"); // red -> green
            if !m.is_in("green") {
                return Ok(m.current_state().clone()); // 失敗
            }
            
            Ok(m.current_state().clone())
        })
        .then(|m| m.is_in("green")); // 最終的にgreenに戻っていることを確認
    
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