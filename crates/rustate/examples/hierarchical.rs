use rustate::{Action, ActionType, Context, Machine, MachineBuilder, State, Transition};

fn main() -> rustate::Result<()> {
    // Create a hierarchical state machine for a music player
    let mut machine = create_player()?;

    println!("Music player state machine created");
    println!("Current states: {:?}", machine.current_states);

    // Send some events
    println!("\nSending PLAY event");
    machine.send("PLAY")?;
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending NEXT event");
    machine.send("NEXT")?;
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending PAUSE event");
    machine.send("PAUSE")?;
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending PLAY event");
    machine.send("PLAY")?;
    println!("Current states: {:?}", machine.current_states);

    println!("\nSending POWER event");
    machine.send("POWER")?;
    println!("Current states: {:?}", machine.current_states);

    Ok(())
}

fn create_player() -> rustate::Result<Machine> {
    // Create states
    let power_off = State::new("powerOff");

    let mut player = State::new_compound("player", "stopped");
    player.parent = Some("root".to_string());

    let mut stopped = State::new("stopped");
    stopped.parent = Some("player".to_string());

    let mut playing = State::new_compound("playing", "normal");
    playing.parent = Some("player".to_string());

    let mut normal = State::new("normal");
    normal.parent = Some("playing".to_string());

    let mut double_speed = State::new("doubleSpeed");
    double_speed.parent = Some("playing".to_string());

    let mut paused = State::new("paused");
    paused.parent = Some("player".to_string());

    // Create transitions
    let power_toggle = Transition::new("powerOff", "POWER", "player");
    let power_off_transition = Transition::new("player", "POWER", "powerOff");

    let play = Transition::new("stopped", "PLAY", "playing");
    let stop = Transition::new("playing", "STOP", "stopped");
    let pause = Transition::new("playing", "PAUSE", "paused");
    let resume = Transition::new("paused", "PLAY", "playing");

    let speed_up = Transition::new("normal", "SPEED_UP", "doubleSpeed");
    let speed_normal = Transition::new("doubleSpeed", "SPEED_NORMAL", "normal");

    let next_track = Transition::internal_transition("playing", "NEXT");
    let prev_track = Transition::internal_transition("playing", "PREV");

    // Create guards and actions
    let log_power_on = Action::new("logPowerOn", ActionType::Entry, |_ctx, _evt| {
        println!("Power ON - Player ready")
    });

    let log_power_off = Action::new("logPowerOff", ActionType::Entry, |_ctx, _evt| {
        println!("Power OFF")
    });

    let log_playing = Action::new("logPlaying", ActionType::Entry, |_ctx, _evt| {
        println!("Playing track")
    });

    let log_stopped = Action::new("logStopped", ActionType::Entry, |_ctx, _evt| {
        println!("Stopped")
    });

    let log_paused = Action::new("logPaused", ActionType::Entry, |_ctx, _evt| {
        println!("Paused")
    });

    let log_double_speed = Action::new("logDoubleSpeed", ActionType::Entry, |_ctx, _evt| {
        println!("Playing at double speed")
    });

    let log_normal_speed = Action::new("logNormalSpeed", ActionType::Entry, |_ctx, _evt| {
        println!("Playing at normal speed")
    });

    let next_track_action = Action::new("nextTrack", ActionType::Transition, |ctx, _evt| {
        let current_track = ctx.get::<usize>("track").unwrap_or(0);
        let next_track = current_track + 1;
        println!("Changing to track {}", next_track);
        let _ = ctx.set("track", next_track);
    });

    let prev_track_action = Action::new("prevTrack", ActionType::Transition, |ctx, _evt| {
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
    let mut next_track = next_track;
    next_track.with_action(next_track_action);

    let mut prev_track = prev_track;
    prev_track.with_action(prev_track_action);

    let machine = MachineBuilder::new("musicPlayer")
        .initial("powerOff")
        .state(power_off)
        .state(player)
        .state(stopped)
        .state(playing)
        .state(normal)
        .state(double_speed)
        .state(paused)
        .transition(power_toggle)
        .transition(power_off_transition)
        .transition(play)
        .transition(stop)
        .transition(pause)
        .transition(resume)
        .transition(speed_up)
        .transition(speed_normal)
        .transition(next_track)
        .transition(prev_track)
        .on_entry("player", log_power_on)
        .on_entry("powerOff", log_power_off)
        .on_entry("playing", log_playing)
        .on_entry("stopped", log_stopped)
        .on_entry("paused", log_paused)
        .on_entry("doubleSpeed", log_double_speed)
        .on_entry("normal", log_normal_speed)
        .context(context)
        .build()?;

    Ok(machine)
}
