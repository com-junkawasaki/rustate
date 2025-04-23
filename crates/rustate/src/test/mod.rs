pub mod checker;
pub mod generator;
pub mod property;
pub mod runner;
#[cfg(feature = "xstate-compat")]
pub mod xstate;

pub use checker::{ModelChecker, Property, PropertyType, VerificationResult};
pub use generator::{TestCase, TestGenerator};
pub use runner::{CoverageReport, TestResult, TestResults, TestRunner};

// プロパティベースドテストモジュールからneed-to-exportのみをexport
#[cfg(feature = "property-testing")]
pub use property::{
    EventSequenceStrategyBuilder, PropertyTestResult, PropertyTestRunner, StateMachineProperty,
};

// XState v5 互換モジュールをexport
#[cfg(feature = "xstate-compat")]
pub use xstate::{
    create_test_model, execute_test_plan, XStatePathSegment, XStateTestModel, XStateTestPath,
    XStateTestPlan,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, ActionType, Machine, MachineBuilder, State, Transition};
    use tokio;

    #[tokio::test]
    async fn it_works() {
        let mut machine = create_test_machine().await;

        assert!(machine.is_in("idle"));

        let result = machine.send("START").await;
        assert!(result.is_ok());

        assert!(machine.is_in("running"));
    }

    async fn create_test_machine() -> Machine {
        let idle_state = State::new("idle");
        let running_state = State::new("running");
        let completed_state = State::new("completed");

        let increment_action = Action::new(
            "incrementCounter",
            ActionType::Transition,
            |ctx, _evt| async move {
                let counter = ctx.get::<i32>("counter").unwrap_or(0);
                let _ = ctx.set("counter", counter + 1);
            },
        );

        let mut start_transition = Transition::new("idle", "START", "running");
        start_transition.with_action(increment_action);

        let complete_transition = Transition::new("running", "COMPLETE", "completed");
        let reset_transition = Transition::new("completed", "RESET", "idle");

        let machine_builder = MachineBuilder::new("testMachine")
            .state(idle_state)
            .state(running_state)
            .state(completed_state)
            .initial("idle")
            .transition(start_transition)
            .transition(complete_transition)
            .transition(reset_transition);
        
        let machine = machine_builder.build().await.unwrap();

        machine.with_state_mapper(|id| match id {
            id if id == "idle" => State::new("idle"),
            id if id == "running" => State::new("running"),
            id if id == "completed" => State::new("completed"),
            id if id == "green" => State::new("green"),
            id if id == "yellow" => State::new("yellow"),
            id if id == "red" => State::new("red"),
            _ => State::new(id),
        })
    }

    #[tokio::test]
    async fn test_generator_all_states() {
        let machine = create_test_machine().await;
        let mut generator = TestGenerator::new(&machine);

        let test_cases = generator.generate_all_states();

        assert_eq!(test_cases.len(), 3);
    }

    #[tokio::test]
    async fn test_generator_all_transitions() {
        let machine = create_test_machine().await;
        let mut generator = TestGenerator::new(&machine);

        let test_cases = generator.generate_all_transitions();

        assert_eq!(test_cases.len(), 3);
    }

    #[tokio::test]
    async fn test_runner_execute_test() {
        let machine = create_test_machine().await;
        let mut runner = TestRunner::new(&machine);

        let test_case = TestCase {
            name: "Idle to Running".to_string(),
            initial_state: "idle".to_string(),
            events: vec![crate::Event::new("START")],
            expected_state: "running".to_string(),
        };

        let result = runner.run_test(&test_case);

        assert!(result.success);
        assert_eq!(result.actual_state, "running");
    }

    #[tokio::test]
    async fn test_model_checker_reachability() {
        let machine = create_test_machine().await;
        let mut checker = ModelChecker::new(&machine);

        let property = Property {
            name: "Can reach completed".to_string(),
            property_type: PropertyType::Reachability,
            target_states: vec!["completed".to_string()],
            description: None,
        };

        let result = checker.verify_property(&property);

        assert!(result.satisfied);
    }

    #[tokio::test]
    async fn test_model_checker_safety() {
        let machine = create_test_machine().await;
        let mut checker = ModelChecker::new(&machine);

        let property = Property {
            name: "Never reach invalid state".to_string(),
            property_type: PropertyType::Safety,
            target_states: vec!["invalid".to_string()],
            description: None,
        };

        let result = checker.verify_property(&property);

        assert!(result.satisfied);
    }
}

#[cfg(test)]
mod advanced_tests {
    use super::*;
    use crate::{
        test::{checker::*, generator::*, runner::*},
        Action, ActionType, Context, Event, Guard, Machine, MachineBuilder, State, Transition,
    };

    #[cfg(feature = "property-testing")]
    use crate::test::property::*;

    fn create_traffic_light_machine() -> Machine {
        let green_state = State::new("green");
        let yellow_state = State::new("yellow");
        let red_state = State::new("red");
        let maintenance_state = State::new("maintenance");

        let timer_guard = Guard::new("timer_guard", |ctx: &Context, _evt: &Event| {
            ctx.get::<i32>("timer").unwrap_or(0) >= 5
        });

        let maintenance_guard = Guard::new("maintenance_guard", |ctx: &Context, _evt: &Event| {
            ctx.get::<bool>("maintenance").unwrap_or(false)
        });

        let increment_timer = Action::new(
            "increment_timer",
            ActionType::Entry,
            |ctx: &mut Context, _evt: &Event| {
                let current = ctx.get::<i32>("timer").unwrap_or(0);
                let _ = ctx.set("timer", current + 1);
            },
        );

        let reset_timer = Action::new(
            "reset_timer",
            ActionType::Transition,
            |ctx: &mut Context, _evt: &Event| {
                let _ = ctx.set("timer", 0);
            },
        );

        let set_maintenance = Action::new(
            "set_maintenance",
            ActionType::Transition,
            |ctx: &mut Context, _evt: &Event| {
                let _ = ctx.set("maintenance", true);
            },
        );

        let clear_maintenance = Action::new(
            "clear_maintenance",
            ActionType::Transition,
            |ctx: &mut Context, _evt: &Event| {
                let _ = ctx.set("maintenance", false);
            },
        );

        let mut green_to_yellow = Transition::new("green", "TIMER", "yellow");
        green_to_yellow.with_guard(timer_guard.clone());
        green_to_yellow.with_action(reset_timer.clone());

        let mut yellow_to_red = Transition::new("yellow", "TIMER", "red");
        yellow_to_red.with_guard(timer_guard.clone());
        yellow_to_red.with_action(reset_timer.clone());

        let mut red_to_green = Transition::new("red", "TIMER", "green");
        red_to_green.with_guard(timer_guard.clone());
        red_to_green.with_action(reset_timer.clone());

        let mut to_maintenance = Transition::new("*", "MAINTENANCE", "maintenance");
        to_maintenance.with_action(set_maintenance);

        let mut from_maintenance = Transition::new("maintenance", "RESTORE", "green");
        from_maintenance.with_action(clear_maintenance);

        let machine = MachineBuilder::new("trafficLight")
            .state(green_state)
            .state(yellow_state)
            .state(red_state)
            .state(maintenance_state)
            .initial("green")
            .transition(green_to_yellow)
            .transition(yellow_to_red)
            .transition(red_to_green)
            .transition(to_maintenance)
            .transition(from_maintenance)
            .on_entry("green", increment_timer.clone())
            .on_entry("yellow", increment_timer.clone())
            .on_entry("red", increment_timer.clone())
            .build()
            .unwrap();

        machine
    }

    #[test]
    fn test_traffic_light_cycle() {
        let mut machine = create_traffic_light_machine();
        assert!(machine.is_in("green"));

        for _ in 0..10 {
            machine.send("TIMER").unwrap();
        }
        assert!(machine.is_in("yellow"));

        for _ in 0..10 {
            machine.send("TIMER").unwrap();
        }
        assert!(machine.is_in("red"));

        for _ in 0..10 {
            machine.send("TIMER").unwrap();
        }
        assert!(machine.is_in("green"));
    }

    #[test]
    fn test_maintenance_mode() {
        let mut machine = create_traffic_light_machine();

        let mut ctx = Context::new();
        ctx.set("maintenance", false).unwrap();
        machine.context = ctx;

        assert!(machine.is_in("green"));

        println!("Available transitions:");
        for transition in &machine.transitions {
            println!(
                "  Source: {}, Event: {}, Target: {:?}",
                transition.source, transition.event, transition.target
            );
        }

        let current_state = machine.current_states.clone();
        println!("Current states before MAINTENANCE: {:?}", current_state);

        let result = machine.send("MAINTENANCE");
        println!("MAINTENANCE event result: {:?}", result);

        let current_state = machine.current_states.clone();
        println!("Current states after MAINTENANCE: {:?}", current_state);

        assert!(machine.is_in("maintenance"));

        machine.send("RESTORE").unwrap();
        assert!(machine.is_in("green"));

        for _ in 0..10 {
            machine.send("TIMER").unwrap();
        }
        assert!(machine.is_in("yellow"));

        machine.send("MAINTENANCE").unwrap();
        assert!(machine.is_in("maintenance"));

        machine.send("RESTORE").unwrap();
        assert!(machine.is_in("green"));
    }

    /*
    #[test]
    fn test_model_checking_traffic_light() {
        let machine = create_traffic_light_machine();
        let mut checker = ModelChecker::new(&machine);

        let all_states_reachable = Property {
            name: "All states are reachable".to_string(),
            property_type: PropertyType::Reachability,
            target_states: vec![
                "green".to_string(),
                "yellow".to_string(),
                "red".to_string(),
                "maintenance".to_string()
            ],
            description: Some("All defined states should be reachable".to_string()),
        };

        let result = checker.verify_property(&all_states_reachable);
        assert!(result.satisfied, "Not all states are reachable: {:#?}", result);

        let invalid_states = Property {
            name: "No invalid states".to_string(),
            property_type: PropertyType::Safety,
            target_states: vec!["invalid".to_string(), "error".to_string()],
            description: Some("System should never reach undefined states".to_string()),
        };

        let result = checker.verify_property(&invalid_states);
        assert!(result.satisfied, "System can reach invalid states: {:#?}", result);
    }
    */

    /*
    #[test]
    fn test_generate_test_cases_for_traffic_light() {
        let machine = create_traffic_light_machine();
        let mut generator = TestGenerator::new(&machine);

        let state_tests = generator.generate_all_states();
        assert_eq!(state_tests.len(), 4, "Should generate test for all 4 states");

        let transition_tests = generator.generate_all_transitions();
        assert_eq!(transition_tests.len(), 5, "Should generate test for all 5 transitions");

        let mut runner = TestRunner::new(&machine);
        let results = runner.run_tests(transition_tests);

        assert!(results.all_passed(), "Not all transition tests passed: {:?}", results);
    }
    */
}
