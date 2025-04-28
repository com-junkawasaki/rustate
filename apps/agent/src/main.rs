use rustate::{machine::Machine, state::State, transition::Transition};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc; // Import MPSC channel
use tracing::info;

// --- State Machine Definition ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TodoState {
    Idle,
    AddingTodo { title: String },
    ViewingTodos,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TodoEvent {
    Add { title: String, content: String },
    View,
    Added { id: u32 },
    Viewed { count: usize },
    BackToIdle,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TodoContext {
    todos: Vec<TodoItem>,
    last_added_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    id: u32,
    title: String,
    content: String,
    completed: bool,
}

implement State<TodoState, TodoEvent, TodoContext> for TodoState {
    fn name(&self) -> &'static str {
        match self {
            TodoState::Idle => "Idle",
            TodoState::AddingTodo { .. } => "AddingTodo",
            TodoState::ViewingTodos => "ViewingTodos",
        }
    }

    fn transitions(&self) -> Vec<Transition<TodoState, TodoEvent, TodoContext>> {
        match self {
            TodoState::Idle => vec![
                Transition::new(
                    |event, _ctx| matches!(event, TodoEvent::Add { .. }),
                    |event, _ctx| {
                        if let TodoEvent::Add { title, .. } = event {
                            Some(TodoState::AddingTodo { title: title.clone() })
                        } else {
                            None
                        }
                    },
                )
                .description("Start adding a todo"),
                Transition::new(
                    |event, _ctx| matches!(event, TodoEvent::View),
                    |_event, _ctx| Some(TodoState::ViewingTodos),
                )
                .description("View existing todos"),
            ],
            TodoState::AddingTodo { .. } => vec![Transition::new(
                |event, _ctx| matches!(event, TodoEvent::Added { .. }),
                |_event, _ctx| Some(TodoState::Idle),
            )
            .description("Finish adding todo (transition triggered by Added event)")],
            TodoState::ViewingTodos => vec![Transition::new(
                |event, _ctx| matches!(event, TodoEvent::Viewed { .. }),
                |_event, _ctx| Some(TodoState::Idle),
            )
            .description("Finish viewing todos (transition triggered by Viewed event)")],
        }
    }

    fn entry_action(&self, event: &TodoEvent, context: &mut TodoContext) {
        info!("Entering state: {:?} due to event {:?}", self.name(), event);
        match self {
            TodoState::AddingTodo { title } => {
                 if let TodoEvent::Add { content, .. } = event {
                    // Prepare context immediately for the task to use
                    context.last_added_id += 1;
                    let new_todo = TodoItem {
                       id: context.last_added_id,
                       title: title.clone(),
                       content: content.clone(),
                       completed: false,
                    };
                    context.todos.push(new_todo);
                    info!("Context updated for Add: ID {}, Title: {}", context.last_added_id, title);
                 }
            }
            TodoState::ViewingTodos => {
                info!("Viewing todos state entered. Count: {}", context.todos.len());
            }
            TodoState::Idle => {
                 if let TodoEvent::Added { id } = event {
                      info!("Returned to Idle after adding todo {}", id);
                 } else if let TodoEvent::Viewed { count } = event {
                      info!("Returned to Idle after viewing {} todos", count);
                 }
            }
        }
    }

     fn exit_action(&self, event: &TodoEvent, _context: &mut TodoContext) {
         info!("Exiting state: {:?} due to event {:?}", self.name(), event);
     }
}

// --- Agent Definition ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    prev_state: Option<String>, // Store state name
    event: String,          // Store event representation
    current_state: String,  // Store state name
    // Add context snapshot, timestamp, etc. as needed
}

#[derive(Debug)]
pub struct Agent<S, E, C>
where
    S: State<S, E, C> + Send + Sync + 'static + Clone + Serialize + std::fmt::Debug,
    E: Send + Sync + 'static + Clone + Serialize + std::fmt::Debug,
    C: Send + Sync + 'static + Default + Clone + Serialize + std::fmt::Debug,
{
    name: String,
    // llm_client: SomeLLMClient, // Placeholder for actual LLM integration
    observations: Arc<Mutex<Vec<Observation>>>,
    // Add memory for Feedback, Plans, etc.
    _phantom: std::marker::PhantomData<(S, E, C)>,
}

impl<S, E, C> Agent<S, E, C>
where
    S: State<S, E, C> + Send + Sync + 'static + Clone + Serialize + std::fmt::Debug,
    E: Send + Sync + 'static + Clone + Serialize + std::fmt::Debug,
    C: Send + Sync + 'static + Default + Clone + Serialize + std::fmt::Debug,
{
    pub fn new(name: String) -> Self {
        Agent {
            name,
            observations: Arc::new(Mutex::new(Vec::new())),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Records an observation of a state transition.
    pub fn add_observation(&self, prev_state: Option<&S>, event: &E, current_state: &S) {
        let observation = Observation {
            prev_state: prev_state.map(|s| s.name().to_string()),
            event: format!("{:?}", event), // Simple representation
            current_state: current_state.name().to_string(),
        };
        info!("Agent '{}' observed: {:?}", self.name, observation);
        self.observations.lock().unwrap().push(observation);
    }

    /// Placeholder for the agent's decision-making logic.
    pub async fn decide(&self, current_state: &S, context: &C, goal: &str) -> Option<E> {
        info!(
            "Agent '{}' deciding based on goal: '{}'. Current state: {}. Context: {:?}",
            self.name,
            goal,
            current_state.name(), // Log state name for brevity
            context
        );
        // --- Simple Rule-Based Placeholder --- 
        if goal.contains("add todo") && current_state.name() == "Idle" {
             let parts: Vec<&str> = goal.splitn(3, ' ').collect();
             if parts.len() >= 3 && parts[0] == "add" && parts[1] == "todo" {
                 let title_content: Vec<&str> = parts[2].splitn(2, ':').collect();
                 if title_content.len() == 2 {
                    let title = title_content[0].trim().to_string();
                    let content = title_content[1].trim().to_string();
                     info!("Agent decided to trigger Add event for '{}'", title);
                    // return Some(TodoEvent::Add { title, content }); // Enable this for auto-triggering
                 }
             }
        } else if goal.contains("view todos") && current_state.name() == "Idle" {
             info!("Agent decided to trigger View event");
             // return Some(TodoEvent::View); // Enable this for auto-triggering
        }
        None // No automatic decision in this placeholder by default
    }

    pub fn get_observations(&self) -> Vec<Observation> {
        self.observations.lock().unwrap().clone()
    }
}

// --- Main Interaction Loop ---

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

    let initial_state = TodoState::Idle;
    let initial_context = TodoContext::default();
    let machine = Arc::new(Mutex::new(
        Machine::new("TodoMachine", initial_state, initial_context).unwrap(),
    ));

    let agent = Arc::new(Agent::<TodoState, TodoEvent, TodoContext>::new(
        "TodoAgent".to_string(),
    ));

    // Create an MPSC channel for async tasks to send events back
    let (event_sender, mut event_receiver) = mpsc::channel::<TodoEvent>(32); // Buffer size 32

    let machine_clone_sub = Arc::clone(&machine);
    let agent_clone_sub = Arc::clone(&agent);
    let event_sender_clone_sub = event_sender.clone();

    // Subscribe the agent and async task spawner to state changes
    machine
        .lock()
        .unwrap()
        .subscribe(move |prev_state, event, current_state, context| {
            // Agent observes the transition
            agent_clone_sub.add_observation(prev_state, event, current_state);

            // Spawn async tasks on entering specific states
            let sender_task = event_sender_clone_sub.clone();
            let context_task = context.clone(); // Clone context for the task

            match current_state {
                TodoState::AddingTodo { title } => {
                    let todo_id = context_task.last_added_id; // Get ID from context updated in entry_action
                    let task_title = title.clone();
                    tokio::spawn(async move {
                        info!("Task: Simulating add operation for ID {} ('{}')...", todo_id, task_title);
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await; // Simulate work
                        info!("Task: Add operation complete for ID {}. Sending Added event.", todo_id);
                        if let Err(e) = sender_task.send(TodoEvent::Added { id: todo_id }).await {
                            info!("Error sending Added event from task: {}", e);
                        }
                    });
                }
                TodoState::ViewingTodos => {
                     let count = context_task.todos.len();
                     tokio::spawn(async move {
                         info!("Task: Simulating view operation ({} items)...", count);
                         tokio::time::sleep(tokio::time::Duration::from_secs(1)).await; // Simulate work
                         info!("Task: View operation complete. Sending Viewed event.");
                         if let Err(e) = sender_task.send(TodoEvent::Viewed { count }).await {
                             info!("Error sending Viewed event from task: {}", e);
                         }
                     });
                }
                _ => {}
            }
        });

    info!("Starting interaction loop. Type 'add <title>:<content>', 'view', or 'quit'.");
    info!("Async tasks will send completion events automatically.");

    loop {
        let current_state_name;
        {
             current_state_name = machine.lock().unwrap().current_state().name().to_string();
        } // Release lock

        info!("Current state: {}. Waiting for input or async event...", current_state_name);

        tokio::select! {
            // Wait for user input
            input_result = read_line_async() => {
                match input_result {
                    Ok(input) => {
                        let command = input.trim();
                        if command == "quit" {
                            break;
                        }

                        let event_to_send: Option<TodoEvent> = {
                             let machine_locked = machine.lock().unwrap(); // Lock needed to check current state
                             let state = machine_locked.current_state();
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
                        }; // Release lock

                        if let Some(event) = event_to_send {
                            let mut machine_locked = machine.lock().unwrap();
                            info!("Sending user event: {:?}", event);
                            match machine_locked.send(&event) {
                                Ok(_) => info!("User event sent successfully."),
                                Err(e) => info!("Failed to send user event: {}", e),
                            }
                        }
                    }
                    Err(e) => {
                        info!("Error reading input: {}", e);
                        break;
                    }
                }
            }
            // Wait for events from async tasks
            Some(event) = event_receiver.recv() => {
                info!("Received async event: {:?}", event);
                let mut machine_locked = machine.lock().unwrap();
                match machine_locked.send(&event) {
                    Ok(_) => info!("Async event sent successfully."),
                    Err(e) => info!("Failed to send async event: {}", e),
                }
            }
        }
        // Small delay removed, select! handles waiting
    }

    info!("Interaction ended.");
    info!("Final Observations: {:?}", agent.get_observations());
} 