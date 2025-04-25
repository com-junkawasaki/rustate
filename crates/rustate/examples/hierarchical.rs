use rustate::{
    Action, Context, Event, EventTrait, Machine, MachineBuilder, State, StateTrait, Transition,
    TransitionType,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use tokio; // Need tokio runtime for async main

// Define a custom Event type
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum MusicEvent {
    Power,
    Play,
    Stop,
    Pause,
    Next,
    Prev,
    SpeedUp,
    SpeedNormal,
    // Add a default event if necessary, or handle None case
    None,
}

impl fmt::Display for MusicEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// Implement EventTrait for your custom event enum
impl EventTrait for MusicEvent {
    fn name(&self) -> &str {
        match self {
            MusicEvent::Power => "POWER",
            MusicEvent::Play => "PLAY",
            MusicEvent::Stop => "STOP",
            MusicEvent::Pause => "PAUSE",
            MusicEvent::Next => "NEXT",
            MusicEvent::Prev => "PREV",
            MusicEvent::SpeedUp => "SPEED_UP",
            MusicEvent::SpeedNormal => "SPEED_NORMAL",
            MusicEvent::None => "NONE",
        }
    }
    fn event_type(&self) -> &str {
        self.name()
    }
    fn payload(&self) -> Option<&serde_json::Value> {
        None
    }
}

// Implement Default for the custom event enum
impl Default for MusicEvent {
    fn default() -> Self {
        MusicEvent::None // Provide a sensible default
    }
}


// Use String for StateTrait
type PlayerState = String;

#[tokio::main] // Use tokio::main for async
async fn main() -> rustate::Result<()> {
    // Create a hierarchical state machine for a music player
    let mut machine = create_player().await?; // Await the async builder

    println!("Music player state machine created");
    println!("Current states: {:?}", machine.current_states);

    // Send some events
    println!("\nSending PLAY event");
    machine.send(MusicEvent::Play).await?; // Use custom event enum and await
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending NEXT event");
    machine.send(MusicEvent::Next).await?;
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending PAUSE event");
    machine.send(MusicEvent::Pause).await?;
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending PLAY event");
    machine.send(MusicEvent::Play).await?;
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending POWER event");
    machine.send(MusicEvent::Power).await?;
    println!("Current states: {:?}", machine.current_states);

    Ok(())
}

// Make create_player async
async fn create_player() -> rustate::Result<Machine<Context, MusicEvent, PlayerState>> {
    // Use String for State IDs
    let power_off = State::new("powerOff".to_string());

    // Initial state must be provided to new_compound
    let mut player = State::new_compound("player".to_string(), "stopped".to_string());
    // Parent is set implicitly when adding to StateCollection or MachineBuilder (no direct parent field)

    let stopped = State::new("stopped".to_string());
    // stopped.parent = Some("player".to_string()); // Remove manual parent setting

    // Initial state must be provided to new_compound
    let mut playing = State::new_compound("playing".to_string(), "normal".to_string());
    // playing.parent = Some("player".to_string()); // Remove manual parent setting

    let normal = State::new("normal".to_string());
    // normal.parent = Some("playing".to_string()); // Remove manual parent setting

    let double_speed = State::new("doubleSpeed".to_string());
    // double_speed.parent = Some("playing".to_string()); // Remove manual parent setting

    let paused = State::new("paused".to_string());
    // paused.parent = Some("player".to_string()); // Remove manual parent setting

    // Create transitions with full arguments (source, event, target, guard, actions, type)
    let power_toggle = Transition::new(
        "powerOff".to_string(),
        MusicEvent::Power,
        Some("player".to_string()),
        None, // guard
        vec![], // actions
        TransitionType::External,
    );
    let power_off_transition = Transition::new(
        "player".to_string(),
        MusicEvent::Power,
        Some("powerOff".to_string()),
        None,
        vec![],
        TransitionType::External,
    );

    let play = Transition::new(
        "stopped".to_string(),
        MusicEvent::Play,
        Some("playing".to_string()),
        None,
        vec![],
        TransitionType::External,
    );
    let stop = Transition::new(
        "playing".to_string(),
        MusicEvent::Stop,
        Some("stopped".to_string()),
        None,
        vec![],
        TransitionType::External,
    );
    let pause = Transition::new(
        "playing".to_string(),
        MusicEvent::Pause,
        Some("paused".to_string()),
        None,
        vec![],
        TransitionType::External,
    );
    let resume = Transition::new(
        "paused".to_string(),
        MusicEvent::Play,
        Some("playing".to_string()),
        None,
        vec![],
        TransitionType::External,
    );

    let speed_up = Transition::new(
        "normal".to_string(),
        MusicEvent::SpeedUp,
        Some("doubleSpeed".to_string()),
        None,
        vec![],
        TransitionType::External,
    );
    let speed_normal = Transition::new(
        "doubleSpeed".to_string(),
        MusicEvent::SpeedNormal,
        Some("normal".to_string()),
        None,
        vec![],
        TransitionType::External,
    );

    // Internal transitions need actions added separately if needed
    let next_track_trans = Transition::new(
        "playing".to_string(), // Source state for internal transition
        MusicEvent::Next,
        None, // No target for internal
        None,
        vec![], // Actions will be added later
        TransitionType::Internal,
    );
    let prev_track_trans = Transition::new(
        "playing".to_string(),
        MusicEvent::Prev,
        None,
        None,
        vec![], // Actions will be added later
        TransitionType::Internal,
    );

    // Create guards and actions using Action::from_fn
    let log_power_on = Action::from_fn(|_ctx: &mut Context, _evt: &MusicEvent| async {
        println!("Power ON - Player ready")
    });

    let log_power_off = Action::from_fn(|_ctx: &mut Context, _evt: &MusicEvent| async {
        println!("Power OFF")
    });

    let log_playing = Action::from_fn(|_ctx: &mut Context, _evt: &MusicEvent| async {
        println!("Playing track")
    });

    let log_stopped = Action::from_fn(|_ctx: &mut Context, _evt: &MusicEvent| async {
        println!("Stopped")
    });

    let log_paused = Action::from_fn(|_ctx: &mut Context, _evt: &MusicEvent| async {
        println!("Paused")
    });

    let log_double_speed = Action::from_fn(|_ctx: &mut Context, _evt: &MusicEvent| async {
        println!("Playing at double speed")
    });

    let log_normal_speed = Action::from_fn(|_ctx: &mut Context, _evt: &MusicEvent| async {
        println!("Playing at normal speed")
    });

    let next_track_action = Action::from_fn(|ctx: &mut Context, _evt: &MusicEvent| async {
        let current_track = ctx.get::<usize>("track").unwrap_or(0);
        let next_track = current_track + 1;
        println!("Changing to track {}", next_track);
        let _ = ctx.set("track", next_track);
    });

    let prev_track_action = Action::from_fn(|ctx: &mut Context, _evt: &MusicEvent| async {
        let current_track = ctx.get::<usize>("track").unwrap_or(0);
        let prev_track = if current_track > 0 {
            current_track - 1
        } else {
            0
        };
        println!("Changing to track {}", prev_track);
        let _ = ctx.set("track", prev_track);
    });

    // Create context with initial track
    let mut context = Context::new();
    let _ = context.set("track", 0);

    // Create and configure the state machine
    // Add actions directly to transitions or using on_entry/on_exit
    let next_track_trans_with_action = next_track_trans.with_action(next_track_action);
    let prev_track_trans_with_action = prev_track_trans.with_action(prev_track_action);

    // MachineBuilder::new needs the initial state ID
    let machine = MachineBuilder::<Context, MusicEvent, PlayerState>::new(
        "musicPlayer",
        "powerOff".to_string(), // Provide initial state ID here
    )
    // Removed .initial("powerOff") - it's now a required arg for new()
    .state(power_off)
    // Add player state with its children explicitly defined
    .state(
        player
            .with_child(stopped.clone()) // Add children using with_child
            .with_child(playing.clone())
            .with_child(paused.clone()),
    )
    // Add playing state with its children
    .state(
        playing
            .with_child(normal.clone())
            .with_child(double_speed.clone()),
    )
    // Add leaf states (stopped, paused, normal, doubleSpeed were cloned above)
    .state(stopped)
    .state(paused)
    .state(normal)
    .state(double_speed)
    // Add transitions
    .transition(power_toggle)
    .transition(power_off_transition)
    .transition(play)
    .transition(stop)
    .transition(pause)
    .transition(resume)
    .transition(speed_up)
    .transition(speed_normal)
    .transition(next_track_trans_with_action) // Use transition with action
    .transition(prev_track_trans_with_action) // Use transition with action
    .on_entry(&"player".to_string(), log_power_on) // Use state ID directly
    .on_entry(&"powerOff".to_string(), log_power_off)
    .on_entry(&"playing".to_string(), log_playing)
    .on_entry(&"stopped".to_string(), log_stopped)
    .on_entry(&"paused".to_string(), log_paused)
    .on_entry(&"doubleSpeed".to_string(), log_double_speed)
    .on_entry(&"normal".to_string(), log_normal_speed)
    .context(context)
    .build() // Build is now async
    .await?; // Await the build result

    Ok(machine)
}
