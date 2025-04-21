pub mod checker;
pub mod generator;
pub mod runner;
pub mod property;

pub use checker::{ModelChecker, Property, PropertyType, VerificationResult};
pub use generator::{TestCase, TestGenerator};
pub use runner::{CoverageReport, TestResult, TestResults, TestRunner};

// プロパティベースドテストモジュールからneed-to-exportのみをexport
#[cfg(feature = "property-testing")]
pub use property::{PropertyTestResult, PropertyTestRunner, StateMachineProperty, EventSequenceStrategyBuilder};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, ActionType, Context, Event, Machine, MachineBuilder, State, Transition};

    #[test]
    fn it_works() {
        // シンプルなステートマシンを作成
        let mut machine = create_test_machine();
        
        // 初期状態を確認
        assert!(machine.is_in("idle"));
        
        // イベントを送信
        let result = machine.send("START");
        assert!(result.is_ok());
        assert!(result.unwrap());
        
        // 状態が遷移したことを確認
        assert!(machine.is_in("running"));
        
        // コンテキストの値を確認
        assert_eq!(machine.context.get::<i32>("counter").unwrap_or(0), 1);
    }
    
    fn create_test_machine() -> Machine {
        // 状態定義
        let idle_state = State::new("idle");
        let running_state = State::new("running");
        
        // カウンターをインクリメントするアクション
        let increment_action = Action::new(
            "incrementCounter",
            ActionType::Transition,
            |ctx, _evt| {
                let counter = ctx.get::<i32>("counter").unwrap_or(0);
                let _ = ctx.set("counter", counter + 1);
            },
        );
        
        // 遷移を定義
        let mut start_transition = Transition::new("idle", "START", "running");
        start_transition.with_action(increment_action);
        
        // マシンを構築
        MachineBuilder::new("testMachine")
            .state(idle_state)
            .state(running_state)
            .initial("idle")
            .transition(start_transition)
            .build()
            .unwrap()
    }

    #[test]
    fn test_generator_all_states() {
        let machine = create_test_machine();
        let mut generator = TestGenerator::new(&machine);

        let test_cases = generator.generate_all_states();

        // 3つの状態があるはず
        assert_eq!(test_cases.len(), 3);
    }

    #[test]
    fn test_generator_all_transitions() {
        let machine = create_test_machine();
        let mut generator = TestGenerator::new(&machine);

        let test_cases = generator.generate_all_transitions();

        // 3つの遷移があるはず
        assert_eq!(test_cases.len(), 3);
    }

    #[test]
    fn test_runner_execute_test() {
        let machine = create_test_machine();
        let mut runner = TestRunner::new(&machine);

        // グリーンからイエローへの遷移をテスト
        let test_case = TestCase {
            name: "Green to Yellow".to_string(),
            initial_state: "green".to_string(),
            events: vec![crate::Event::new("TIMER")],
            expected_state: "yellow".to_string(),
        };

        let result = runner.run_test(&test_case);

        // テストは成功するはず
        assert!(result.success);
        assert_eq!(result.actual_state, "yellow");
    }

    #[test]
    fn test_model_checker_reachability() {
        let machine = create_test_machine();
        let mut checker = ModelChecker::new(&machine);

        // 到達可能性プロパティをチェック
        let property = Property {
            name: "Can reach red".to_string(),
            property_type: PropertyType::Reachability,
            target_states: vec!["red".to_string()],
            description: None,
        };

        let result = checker.verify_property(&property);

        // redは到達可能なので、プロパティは満たされるはず
        assert!(result.satisfied);
    }

    #[test]
    fn test_model_checker_safety() {
        let machine = create_test_machine();
        let mut checker = ModelChecker::new(&machine);

        // 安全性プロパティをチェック
        let property = Property {
            name: "Never reach invalid state".to_string(),
            property_type: PropertyType::Safety,
            target_states: vec!["invalid".to_string()],
            description: None,
        };

        let result = checker.verify_property(&property);

        // invalidは存在しないので到達不可能、プロパティは満たされるはず
        assert!(result.satisfied);
    }
}
