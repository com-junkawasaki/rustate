mod agent;
mod state_machine;

use agent::Agent;
use rustate::machine::MachineBuilder;
use state_machine::{TodoContext, TodoEvent, TodoState};

use agent::{Feedback, FeedbackOutcome};
use chrono::Utc;
use dotenvy::dotenv;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

// Helper function to read stdin async
async fn read_line_async() -> Result<String, std::io::Error> {
    tokio::task::spawn_blocking(|| {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).map(|_| input)
    })
    .await?
}

// --- Wrapper function for sending events and handling side effects ---
async fn send_event_and_observe(
    machine_arc: &Arc<Mutex<rustate::machine::Machine<TodoContext, TodoEvent, TodoState, ()>>>,
    agent_arc: &Arc<Agent>,
    event_sender: &mpsc::Sender<TodoEvent>,
    event: TodoEvent,
    is_agent_event: bool,
    goal_for_feedback: &str,
) -> Result<(), String> {
    let prev_state_cloned;
    {
        // Lock to get state *before* sending
        let machine_locked = machine_arc.lock().await;
        prev_state_cloned = machine_locked
            .current_states
            .iter()
            .next()
            .expect("Machine has no current state before send")
            .clone();
    }

    // Send the event
    let send_result = {
        let mut machine_locked = machine_arc.lock().await;
        machine_locked.send(event.clone()).await
    };

    let mut outcome_for_feedback: Option<FeedbackOutcome> = None;
    let mut reason_for_feedback: Option<String> = None;

    match send_result {
        Ok(changed) => {
            let current_state_cloned;
            let current_context_arc;
            {
                // Lock again to get the *new* state and context Arc
                let machine_locked = machine_arc.lock().await;
                current_state_cloned = machine_locked
                    .current_states
                    .iter()
                    .next()
                    .expect("Machine has no current state after send")
                    .clone();
                current_context_arc = machine_locked.context.clone();
            }

            // Add observation regardless of source
            agent_arc.add_observation(
                Some(&prev_state_cloned),
                &event, // Use original event here
                &current_state_cloned,
            );

            // Determine feedback outcome for agent events
            if is_agent_event {
                outcome_for_feedback = Some(if changed {
                    FeedbackOutcome::SuccessStateChanged
                } else {
                    FeedbackOutcome::SuccessNoStateChange
                });
            }

            // Spawn tasks based on the new state (only if state changed?)
            if changed {
                let sender_task = event_sender.clone();
                let context_task_arc = current_context_arc;

                match current_state_cloned {
                    TodoState::AddingTodo { ref title } => {
                        let context_guard = context_task_arc.read().await;
                        let todo_id = context_guard.last_added_id;
                        let task_title = title.clone();
                        tokio::spawn(async move {
                            info!(
                                "Task: Simulating add operation for ID {} ('{}')...",
                                todo_id, task_title
                            );
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            info!(
                                "Task: Add operation complete for ID {}. Sending Added event.",
                                todo_id
                            );
                            sender_task
                                .send(TodoEvent::Added { id: todo_id })
                                .await
                                .map_err(|e| error!("Failed to send Added event: {}", e))
                                .ok();
                        });
                    }
                    TodoState::ViewingTodos => {
                        let context_guard = context_task_arc.read().await;
                        let count = context_guard.todos.len();
                        tokio::spawn(async move {
                            info!("Task: Simulating view operation ({} items)...", count);
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            info!("Task: View operation complete. Sending Viewed event.");
                            sender_task
                                .send(TodoEvent::Viewed { count })
                                .await
                                .map_err(|e| error!("Failed to send Viewed event: {}", e))
                                .ok();
                        });
                    }
                    _ => {}
                }
            } else if !is_agent_event {
                // Log if non-agent event didn't change state
                info!("Event {:?} did not cause a state change.", event);
            }
            Ok(())
        }
        Err(e) => {
            if is_agent_event {
                outcome_for_feedback = Some(FeedbackOutcome::SendError(e.to_string()));
                reason_for_feedback = Some(format!("{:?}", e));
            }
            warn!("Failed to send event {:?}: {}", event, e);
            Err(format!("Failed to send event: {}", e))
        }
    }?;

    if is_agent_event {
        if let Some(outcome) = outcome_for_feedback {
            let feedback = Feedback {
                timestamp: Utc::now(),
                attempted_event: Some(event),
                outcome,
                reason: reason_for_feedback,
                goal_at_time: goal_for_feedback.to_string(),
                state_at_time: prev_state_cloned.name().to_string(),
            };
            agent_arc.add_feedback(feedback);
        }
    }

    Ok(())
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
        let _machine_locked = machine.lock().await;
        info!("Subscription temporarily disabled for debugging.");
    }

    info!("Starting interaction loop. Type 'goal <your goal>', 'add ...', 'view', or 'quit'.");
    info!("Agent will attempt to act based on the last specified 'goal'.");

    let mut current_goal = "Manage the todo list".to_string();

    loop {
        let (current_state_cloned, current_context_cloned);
        {
            let machine_locked = machine.lock().await;
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

            maybe_agent_event = agent.decide(
                &current_state_cloned,
                &current_context_cloned,
                &current_goal
            ), if current_state_cloned == TodoState::Idle => {
                if let Some(event) = maybe_agent_event {
                     info!("Agent wants to send event: {:?}", event);
                     // Call the wrapper function
                     if let Err(e) = send_event_and_observe(&machine, &agent, &event_sender, event, true, &current_goal).await {
                         warn!("Error processing agent event: {}", e);
                     }
                } else {
                    info!("Agent decided not to act or failed to decide.");
                }
            }

            Some(event) = event_receiver.recv() => {
                info!("Received async event: {:?}", event);
                 // Call the wrapper function
                 if let Err(e) = send_event_and_observe(&machine, &agent, &event_sender, event, false, &current_goal).await {
                     error!("Error processing async event: {}", e);
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
                             let machine_locked = machine.lock().await;
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
                            info!("Sending user event: {:?}", event);
                            // Call the wrapper function
                            if let Err(e) = send_event_and_observe(&machine, &agent, &event_sender, event, false, &current_goal).await {
                                info!("Error processing user event: {}", e); // Keep as info for user errors?
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
