use rstate::{Action, ActionType, Context, Event, Machine, MachineBuilder, State, Transition};
use std::thread::sleep;
use std::time::Duration;

fn main() -> rstate::Result<()> {
    // Create a traffic light state machine
    let machine = create_traffic_light()?;

    println!("Traffic light state machine created");
    println!("Current state: {:?}", machine.current_states);

    // Run the traffic light simulation
    run_simulation(machine)?;

    Ok(())
}

fn create_traffic_light() -> rstate::Result<Machine> {
    // Create the states
    let green = State::new("green");
    let yellow = State::new("yellow");
    let red = State::new("red");

    // Create the transitions
    let green_to_yellow = Transition::new("green", "TIMER", "yellow");
    let yellow_to_red = Transition::new("yellow", "TIMER", "red");
    let red_to_green = Transition::new("red", "TIMER", "green");

    // Define some actions
    let log_green = Action::new(
        "logGreen",
        ActionType::Entry,
        |_ctx, _evt| println!("Entering GREEN state - Go!"),
    );

    let log_yellow = Action::new(
        "logYellow",
        ActionType::Entry,
        |_ctx, _evt| println!("Entering YELLOW state - Slow down!"),
    );

    let log_red = Action::new(
        "logRed",
        ActionType::Entry,
        |_ctx, _evt| println!("Entering RED state - Stop!"),
    );

    // Build the machine
    let machine = MachineBuilder::new("trafficLight")
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
        .build()?;

    Ok(machine)
}

fn run_simulation(mut machine: Machine) -> rstate::Result<()> {
    println!("\nStarting traffic light simulation...");
    println!("Press Ctrl+C to exit\n");

    loop {
        // Wait some time in the current state
        let wait_time = match machine.current_states.iter().next() {
            Some(state) if state == "green" => 3,
            Some(state) if state == "yellow" => 1,
            Some(state) if state == "red" => 2,
            _ => 1,
        };

        sleep(Duration::from_secs(wait_time));

        // Send a timer event to transition to the next state
        machine.send("TIMER")?;
    }
} 