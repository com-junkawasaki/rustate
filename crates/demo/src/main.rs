use gloo::console::log;
use rustate::{Action, ActionType, Machine, MachineBuilder, State, StateTrait, Transition};
use std::cell::RefCell;
use std::rc::Rc;
use yew::prelude::*;

// MachineView component
#[derive(Properties, Clone)]
pub struct MachineViewProps {
    machine: Rc<RefCell<Machine>>,
}

// Implement manual PartialEq since Machine doesn't implement it
impl PartialEq for MachineViewProps {
    fn eq(&self, _other: &Self) -> bool {
        // We can't compare machines directly, so we'll consider them always equal
        // This is just for Yew's component rendering system
        true
    }
}

#[function_component(MachineView)]
pub fn machine_view(props: &MachineViewProps) -> Html {
    let machine = props.machine.clone();
    let current_state = use_state(|| machine.borrow().current_state().id().to_string());
    let history = use_state(Vec::new);

    let on_send_event = {
        let machine = machine.clone();
        let current_state = current_state.clone();
        let history = history.clone();

        move |event: String| {
            log!("Sending event:", &event);

            // Add event to history
            history.set({
                let mut new_history = (*history).clone();
                new_history.push(format!("Event: {}", event));
                new_history
            });

            match machine.borrow_mut().send(&event) {
                Ok(_) => {
                    let new_state = machine.borrow().current_state().id().to_string();
                    log!("Transitioned to:", &new_state);

                    // Add state change to history
                    history.set({
                        let mut new_history = (*history).clone();
                        new_history.push(format!("State changed to: {}", new_state));
                        new_history
                    });

                    current_state.set(new_state);
                }
                Err(e) => {
                    log!("Error sending event:", e.to_string());

                    // Add error to history
                    history.set({
                        let mut new_history = (*history).clone();
                        new_history.push(format!("Error: {}", e));
                        new_history
                    });
                }
            }
        }
    };

    html! {
        <div class="state-machine">
            <div class="state-display">
                <h3>{"Current State:"}</h3>
                <div class="current-state">{ (*current_state).clone() }</div>
            </div>

            <div class="controls">
                <button onclick={let cb = on_send_event.clone(); move |_| cb("TIMER".to_string())}>
                    { "Send TIMER Event" }
                </button>
                <button onclick={let cb = on_send_event.clone(); move |_| cb("POWER".to_string())}>
                    { "Send POWER Event" }
                </button>
            </div>

            <div class="machine-history">
                <h3>{"Machine History:"}</h3>
                <ul>
                    { for history.iter().map(|item| html! { <li>{ item }</li> }) }
                </ul>
            </div>
        </div>
    }
}

// Demo App
#[function_component(App)]
pub fn app() -> Html {
    // Create traffic light state machine
    let machine = {
        // Create states
        let green = State::new("green");
        let yellow = State::new("yellow");
        let red = State::new("red");
        let off = State::new("off");

        // Create transitions
        let green_to_yellow = Transition::new("green", "TIMER", "yellow");
        let yellow_to_red = Transition::new("yellow", "TIMER", "red");
        let red_to_green = Transition::new("red", "TIMER", "green");

        // Power transitions
        let on_to_off = Transition::new("green", "POWER", "off");
        let off_to_on = Transition::new("off", "POWER", "green");

        // Define actions
        let log_green = Action::new("logGreen", ActionType::Entry, |_ctx, _evt| {
            log!("Entering GREEN state - Go!")
        });

        let log_yellow = Action::new("logYellow", ActionType::Entry, |_ctx, _evt| {
            log!("Entering YELLOW state - Prepare to stop!")
        });

        let log_red = Action::new("logRed", ActionType::Entry, |_ctx, _evt| {
            log!("Entering RED state - Stop!")
        });

        let log_off = Action::new("logOff", ActionType::Entry, |_ctx, _evt| {
            log!("Entering OFF state - Traffic light is off!")
        });

        // Build the machine
        let machine = MachineBuilder::new("trafficLight")
            .state(green)
            .state(yellow)
            .state(red)
            .state(off)
            .initial("green")
            .transition(green_to_yellow)
            .transition(yellow_to_red)
            .transition(red_to_green)
            .transition(on_to_off)
            .transition(off_to_on)
            .on_entry("green", log_green)
            .on_entry("yellow", log_yellow)
            .on_entry("red", log_red)
            .on_entry("off", log_off)
            .build()
            .unwrap();

        Rc::new(RefCell::new(machine))
    };

    // Create simple machine
    let simple_machine = {
        // Create states
        let state1 = State::new("state1");
        let state2 = State::new("state2");

        // Create transitions
        let state1_to_state2 = Transition::new("state1", "NEXT", "state2");
        let state2_to_state1 = Transition::new("state2", "PREV", "state1");

        // Build the machine
        let machine = MachineBuilder::new("simpleMachine")
            .state(state1)
            .state(state2)
            .initial("state1")
            .transition(state1_to_state2)
            .transition(state2_to_state1)
            .build()
            .unwrap();

        Rc::new(RefCell::new(machine))
    };

    html! {
        <>
            <header>
                <h1>{"RuState Demo"}</h1>
                <p>{"A demonstration of RuState state machines"}</p>
            </header>

            <div class="container">
                <div class="demo-section">
                    <h2>{"Traffic Light State Machine"}</h2>
                    <p>{"A simple state machine simulating a traffic light with states: Green, Yellow, Red, and Off."}</p>
                    <MachineView machine={machine.clone()} />
                </div>

                <div class="demo-section">
                    <h2>{"Simple State Machine"}</h2>
                    <p>{"A basic state machine with simple state transitions."}</p>
                    <MachineView machine={simple_machine.clone()} />
                </div>
            </div>
        </>
    }
}

// We no longer need the main function, as it's moved to lib.rs
// The main function still exists for standalone binary usage
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("This demo is designed for WebAssembly and can't run directly as a binary.");
}

// When compiling for wasm, we keep an empty main function
#[cfg(target_arch = "wasm32")]
fn main() {}
