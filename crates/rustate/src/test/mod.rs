pub mod checker;
pub mod generator;
pub mod runner;

pub use checker::{ModelChecker, Property, PropertyType, VerificationResult};
pub use generator::{TestCase, TestGenerator};
pub use runner::{CoverageReport, TestResult, TestResults, TestRunner};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Machine, MachineBuilder, State, Transition};

    // テスト用の簡単な状態マシンを作成
    fn create_test_machine() -> Machine {
        // 状態の作成
        let green = State::new("green");
        let yellow = State::new("yellow");
        let red = State::new("red");

        // 遷移の作成
        let green_to_yellow = Transition::new("green", "TIMER", "yellow");
        let yellow_to_red = Transition::new("yellow", "TIMER", "red");
        let red_to_green = Transition::new("red", "TIMER", "green");

        // マシンの構築
        MachineBuilder::new("trafficLight")
            .state(green)
            .state(yellow)
            .state(red)
            .initial("green")
            .transition(green_to_yellow)
            .transition(yellow_to_red)
            .transition(red_to_green)
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
