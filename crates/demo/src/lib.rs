use wasm_bindgen::prelude::*;

// mainモジュールを公開
pub mod main;

// Wasm用のパニックフックを設定
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(start)]
pub fn run() {
    init_panic_hook();
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<main::App>::new().render();
}

#[cfg(test)]
mod tests {
    use rustate::{Action, ActionType, Machine, MachineBuilder, State, Transition};
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    // トラフィックライトステートマシンの作成
    fn create_traffic_light_machine() -> Machine {
        // 状態の定義
        let green_state = State::new("green");
        let yellow_state = State::new("yellow");
        let red_state = State::new("red");

        // 遷移の定義
        let green_to_yellow = Transition::new("green", "NEXT", "yellow");
        let yellow_to_red = Transition::new("yellow", "NEXT", "red");
        let red_to_green = Transition::new("red", "NEXT", "green");

        // マシンの構築
        let machine = MachineBuilder::new("trafficLight")
            .state(green_state)
            .state(yellow_state)
            .state(red_state)
            .initial("green")
            .transition(green_to_yellow)
            .transition(yellow_to_red)
            .transition(red_to_green)
            .build()
            .unwrap();

        machine
    }

    #[test]
    fn test_traffic_light_cycle() {
        let mut machine = create_traffic_light_machine();
        
        // 初期状態の確認
        assert!(machine.is_in("green"));
        
        // 状態遷移の確認
        machine.send("NEXT").unwrap();
        assert!(machine.is_in("yellow"));
        
        machine.send("NEXT").unwrap();
        assert!(machine.is_in("red"));
        
        machine.send("NEXT").unwrap();
        assert!(machine.is_in("green"));
    }

    // カウンタマシンの作成
    fn create_counter_machine() -> Machine {
        // 状態の定義
        let active_state = State::new("active");
        
        // アクションの定義
        let increment_action = Action::new("increment", ActionType::Transition, |ctx, _evt| {
            let count = ctx.get::<i32>("count").unwrap_or(0);
            let _ = ctx.set("count", count + 1);
        });
        
        let decrement_action = Action::new("decrement", ActionType::Transition, |ctx, _evt| {
            let count = ctx.get::<i32>("count").unwrap_or(0);
            if count > 0 {
                let _ = ctx.set("count", count - 1);
            }
        });
        
        let reset_action = Action::new("reset", ActionType::Transition, |ctx, _evt| {
            let _ = ctx.set("count", 0);
        });
        
        // 内部遷移の定義（同じ状態へ遷移するが、アクションを実行）
        let mut increment_transition = Transition::new("active", "INCREMENT", "active");
        increment_transition.with_action(increment_action);
        
        let mut decrement_transition = Transition::new("active", "DECREMENT", "active");
        decrement_transition.with_action(decrement_action);
        
        let mut reset_transition = Transition::new("active", "RESET", "active");
        reset_transition.with_action(reset_action);
        
        // マシンの構築
        let machine = MachineBuilder::new("counter")
            .state(active_state)
            .initial("active")
            .transition(increment_transition)
            .transition(decrement_transition)
            .transition(reset_transition)
            .build()
            .unwrap();
        
        machine
    }

    #[test]
    fn test_counter_operations() {
        let mut machine = create_counter_machine();
        
        // 初期状態でカウントが0であることを確認
        assert_eq!(machine.context().get::<i32>("count").unwrap_or(-1), 0);
        
        // インクリメント操作
        for _ in 0..5 {
            machine.send("INCREMENT").unwrap();
        }
        
        // カウントが5になっていることを確認
        assert_eq!(machine.context().get::<i32>("count").unwrap_or(-1), 5);
        
        // デクリメント操作
        for _ in 0..2 {
            machine.send("DECREMENT").unwrap();
        }
        
        // カウントが3になっていることを確認
        assert_eq!(machine.context().get::<i32>("count").unwrap_or(-1), 3);
        
        // リセット操作
        machine.send("RESET").unwrap();
        
        // カウントが0にリセットされていることを確認
        assert_eq!(machine.context().get::<i32>("count").unwrap_or(-1), 0);
    }

    #[test]
    fn test_integration_with_rustate() {
        // ステートマシンの作成
        let mut machine = MachineBuilder::new("testMachine")
            .state(State::new("state1"))
            .state(State::new("state2"))
            .initial("state1")
            .transition(Transition::new("state1", "GOTO_STATE2", "state2"))
            .build()
            .unwrap();
        
        // 初期状態の確認
        assert!(machine.is_in("state1"));
        
        // イベント送信
        let result = machine.send("GOTO_STATE2");
        
        // 遷移成功の確認
        assert!(result.is_ok());
        assert!(machine.is_in("state2"));
    }
}
