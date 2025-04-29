mod agent;
mod state_machine;

use agent::Agent;
use rustate::machine::MachineBuilder;
use state_machine::{TodoContext, TodoEvent, TodoState};

use dotenvy::dotenv;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

// Helper function to read stdin async
async fn read_line_async() -> Result<String, std::io::Error> {
    tokio::task::spawn_blocking(|| {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).map(|_| input)
    })
    .await?
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    if dotenv().is_err() {
        warn!(".env file not found or failed to load. Ensure OPENAI_API_KEY is set in the environment.");
    }

    let agent = match Agent::new("TodoAgent".to_string()) {
        Ok(a) => Arc::new(a),
        Err(e) => {
            error!("Failed to create agent: {}", e);
            return;
        }
    };

    let initial_state = TodoState::Idle;
    let initial_context = TodoContext::default();

    let machine_builder =
        MachineBuilder::<TodoContext, TodoEvent, TodoState, ()>::new("TodoMachine", initial_state)
            .context(initial_context);

    let machine = match machine_builder.build().await {
        Ok(m) => Arc::new(Mutex::<
            rustate::machine::Machine<TodoContext, TodoEvent, TodoState, ()>,
        >::new(m)),
        Err(e) => {
            error!("Failed to build state machine: {}", e);
            return;
        }
    };

    let (event_sender, mut event_receiver) = mpsc::channel::<TodoEvent>(32);

    let _machine_clone_sub = Arc::clone(&machine);
    let _agent_clone_sub = Arc::clone(&agent);
    let _event_sender_clone_sub = event_sender.clone();

    {
        let _machine_locked = machine.lock().unwrap();
        info!("Subscription temporarily disabled for debugging.");
    }

    info!("Starting interaction loop. Type 'goal <your goal>', 'add ...', 'view', or 'quit'.");
    info!("Agent will attempt to act based on the last specified 'goal'.");

    let mut current_goal = "Manage the todo list".to_string();

    loop {
        let (current_state_cloned, current_context_cloned);
        {
            let machine_locked = machine.lock().unwrap();
            current_state_cloned = machine_locked
                .current_states
                .iter()
                .next()
                .expect("Machine has no current state")
                .clone();
            current_context_cloned = machine_locked.context.clone();
        }

        info!(
            "Current state: {}. Goal: '{}'. Waiting...",
            current_state_cloned.name(),
            current_goal
        );

        tokio::select! {
            biased;

            maybe_agent_event = async {
                let context_guard = current_context_cloned.read().await;
                agent.decide(&current_state_cloned, &*context_guard, &current_goal).await
            }, if current_state_cloned == TodoState::Idle => {
                if let Some(event) = maybe_agent_event {
                     info!("Agent wants to send event: {:?}", event);
                     let mut machine_locked = machine.lock().unwrap();
                     match machine_locked.send(event.clone()).await {
                         Ok(_) => info!("Agent event sent successfully."),
                         Err(e) => warn!("Agent failed to send event {:?}: {}", event, e),
                     }
                } else {
                    info!("Agent decided not to act or failed to decide.");
                }
            }

            Some(event) = event_receiver.recv() => {
                info!("Received async event: {:?}", event);
                let mut machine_locked = machine.lock().unwrap();
                match machine_locked.send(event.clone()).await {
                    Ok(_) => info!("Async event sent successfully."),
                    Err(e) => error!("Failed to send async event: {}", e),
                }
            }

            input_result = read_line_async() => {
                match input_result {
                    Ok(input) => {
                        let command = input.trim();
                        if command == "quit" {
                            break;
                        }

                        if let Some(goal_text) = command.strip_prefix("goal ") {
                            current_goal = goal_text.trim().to_string();
                            info!("New goal set: {}", current_goal);
                            continue;
                        }

                        let event_to_send: Option<TodoEvent> = {
                             let machine_locked = machine.lock().unwrap();
                             let state = machine_locked
                                .current_states
                                .iter()
                                .next()
                                .expect("Machine has no current state");
                             match command {
                                 "view" if *state == TodoState::Idle => Some(TodoEvent::View),
                                 cmd if cmd.starts_with("add ") && *state == TodoState::Idle => {
                                     let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
                                     if parts.len() == 2 {
                                         let content_parts: Vec<&str> = parts[1].splitn(2, ':').collect();
                                         if content_parts.len() == 2 {
                                              Some(TodoEvent::Add {
                                                 title: content_parts[0].trim().to_string(),
                                                 content: content_parts[1].trim().to_string(),
                                              })
                                         } else {
                                             info!("Invalid add format. Use: add <title>:<content>");
                                             None
                                         }
                                     } else { None }
                                 },
                                  _ => {
                                     info!("Unknown command '{}' or invalid in state '{}'", command, state.name());
                                     None
                                 }
                             }
                        };

                        if let Some(event) = event_to_send {
                            let mut machine_locked = machine.lock().unwrap();
                            info!("Sending user event: {:?}", event);
                            match machine_locked.send(event.clone()).await {
                                Ok(_) => info!("User event sent successfully."),
                                Err(e) => info!("Failed to send user event: {}", e),
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading input: {}", e);
                        break;
                    }
                }
            }
        }
    }

    info!("Interaction ended.");
    info!("Final Observations: {:?}", agent.get_observations());
}
