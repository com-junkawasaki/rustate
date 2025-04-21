use gloo::console::log;
use serde_json::json;
use std::rc::Rc;
use yew::prelude::*;
use rustate::{Action, ActionType, Guard, Machine, MachineBuilder, State, Transition};

// MachineView component
#[derive(Properties, PartialEq)]
struct MachineViewProps {
    machine: Rc<Machine>,
}

#[function_component(MachineView)]
fn machine_view(props: &MachineViewProps) -> Html {
    let machine = props.machine.clone();
    let current_state = use_state(|| machine.current_state().to_string());
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
            
            match machine.send(&event) {
                Ok(_) => {
                    let new_state = machine.current_state().to_string();
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
fn app() -> Html {
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
        let on_to_off = Transition::new("green", "POWER", "off")
            .add_guard(Guard::new("powerCutGuard", |_, _| true));
        let off_to_on = Transition::new("off", "POWER", "green");
        
        // Define actions
        let log_green = Action::new(
            "logGreen",
            ActionType::Entry,
            |_ctx, _evt| log!("Entering GREEN state - Go!"),
        );
        
        let log_yellow = Action::new(
            "logYellow",
            ActionType::Entry,
            |_ctx, _evt| log!("Entering YELLOW state - Prepare to stop!"),
        );
        
        let log_red = Action::new(
            "logRed",
            ActionType::Entry,
            |_ctx, _evt| log!("Entering RED state - Stop!"),
        );
        
        let log_off = Action::new(
            "logOff",
            ActionType::Entry,
            |_ctx, _evt| log!("Entering OFF state - Traffic light is off!"),
        );

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
            
        Rc::new(machine)
    };
    
    // Create hierarchical machine
    let hierarchical_machine = {
        // Create parent state
        let parent = State::new("parent").compound();
        
        // Create child states
        let child1 = State::new("child1");
        let child2 = State::new("child2");
        
        // Add child states to parent
        let parent = parent.add_child(child1).add_child(child2);
        
        // Create transitions
        let child1_to_child2 = Transition::new("child1", "NEXT", "child2");
        let child2_to_child1 = Transition::new("child2", "PREV", "child1");
        
        // Build the machine
        let machine = MachineBuilder::new("hierarchicalMachine")
            .state(parent)
            .initial("child1")
            .transition(child1_to_child2)
            .transition(child2_to_child1)
            .build()
            .unwrap();
            
        Rc::new(machine)
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
                    <h2>{"Hierarchical State Machine"}</h2>
                    <p>{"A hierarchical state machine with a parent state containing two child states."}</p>
                    <MachineView machine={hierarchical_machine.clone()} />
                </div>
            </div>
        </>
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
} 