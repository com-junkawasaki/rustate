use crate::state_machine::{TodoContext, TodoEvent, TodoState};
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client as OpenAiClient,
};
use dotenvy::dotenv;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};

// --- Agent Definition ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    prev_state: Option<String>,
    event: String,
    current_state: String,
    // Consider adding context snapshot here for richer history
}

#[derive(Debug)]
pub struct Agent {
    name: String,
    client: OpenAiClient<OpenAIConfig>,
    observations: Arc<Mutex<Vec<Observation>>>, // Keep observations thread-safe
                                                // Add memory for Feedback, Plans, Messages etc.
}

impl Agent {
    pub fn new(name: String) -> Result<Self, String> {
        // Load .env file if present. Client::new() should automatically
        // pick up OPENAI_API_KEY from the environment.
        dotenv().ok();

        // Client::new() uses environment variable OPENAI_API_KEY by default
        let client = OpenAiClient::new();
        // Check if key was loaded (optional, client calls will fail later if not)
        if env::var("OPENAI_API_KEY").is_err() {
            warn!("OPENAI_API_KEY not found in environment or .env file. OpenAI calls will fail.");
        }

        Ok(Agent {
            name,
            client,
            observations: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn add_observation(
        &self,
        prev_state: Option<&TodoState>,
        event: &TodoEvent,
        current_state: &TodoState,
    ) {
        let observation = Observation {
            prev_state: prev_state.map(|s| s.name().to_string()),
            event: serde_json::to_string(event).unwrap_or_else(|_| format!("{:?}", event)),
            current_state: current_state.name().to_string(),
        };
        info!("Agent '{}' observed: {:?}", self.name, observation);
        let mut obs = self.observations.lock().unwrap();
        obs.push(observation);
        // Optional: Limit history size
        // if obs.len() > 10 { obs.remove(0); }
    }

    pub async fn decide(
        &self,
        current_state: &TodoState,
        context: &TodoContext,
        goal: &str,
    ) -> Option<TodoEvent> {
        let state_name = current_state.name();
        info!(
            "Agent '{}' deciding based on goal: '{}'. Current state: {}.",
            self.name, goal, state_name,
        );

        let context_json = serde_json::to_string_pretty(context)
            .unwrap_or_else(|_| "Context unavailable".to_string());
        let observations_json = serde_json::to_string_pretty(&*self.observations.lock().unwrap())
            .unwrap_or_else(|_| "Observations unavailable".to_string());

        let event_schema_description = r#"
Available event types (output JSON):
- {"type": "Add", "title": "string", "content": "string"} (Use only when in Idle state)
- {"type": "View"} (Use only when in Idle state)
- (Do not generate Added or Viewed events, they are system responses)
"#;

        let system_prompt = format!(
            r#"
You are an agent controlling a Todo state machine.
Your goal is: {}

The current state is: {}
The current context (list of todos) is:
{}
Recent observations (state transitions):
{}
Based on the goal, current state, context, and observations, decide the next *single* event to send to the state machine to progress towards the goal.
Only output a valid JSON object representing one of the allowed events for the current state.
{}
If no action is appropriate or possible now based on the goal and state, output the text "NO_ACTION".
Output *only* the JSON event object or "NO_ACTION".
"#,
            goal, state_name, context_json, observations_json, event_schema_description
        );

        // Restore original message building logic
        let messages = match (
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system_prompt)
                .build(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(format!(
                    "Goal: {}. Current state: {}. What is the next event?",
                    goal, state_name
                ))
                .build(),
        ) {
            (Ok(sys_msg), Ok(usr_msg)) => {
                vec![
                    ChatCompletionRequestMessage::System(sys_msg),
                    ChatCompletionRequestMessage::User(usr_msg),
                ]
            }
            (Err(e), _) => {
                error!("Failed to build system message: {}", e);
                return None;
            }
            (_, Err(e)) => {
                error!("Failed to build user message: {}", e);
                return None;
            }
        };

        let request = match CreateChatCompletionRequestArgs::default()
            .model("gpt-3.5-turbo")
            .messages(messages)
            .max_tokens(150u16)
            .temperature(0.2)
            .build()
        {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to build OpenAI request: {}", e);
                return None;
            }
        };

        info!("Sending request to OpenAI...");
        match self.client.chat().create(request).await {
            Ok(response) => {
                if let Some(choice) = response.choices.get(0) {
                    let response_text = choice.message.content.as_deref().unwrap_or("").trim();
                    info!("OpenAI response: {}", response_text);

                    if response_text == "NO_ACTION" {
                        info!("Agent decided NO_ACTION.");
                        return None;
                    }

                    match serde_json::from_str::<TodoEvent>(response_text) {
                        Ok(event) => {
                            info!("Agent decided event: {:?}", event);
                            match (&event, state_name) {
                                (TodoEvent::Add { .. }, "Idle") => Some(event),
                                (TodoEvent::View, "Idle") => Some(event),
                                _ => {
                                    warn!("LLM proposed event {:?} invalid for current state {}. Ignoring.", event, state_name);
                                    None
                                }
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to parse LLM response as TodoEvent JSON: {}. Response: {}",
                                e, response_text
                            );
                            None
                        }
                    }
                } else {
                    error!("No choices received from OpenAI.");
                    None
                }
            }
            Err(e) => {
                error!("Error calling OpenAI API: {}", e);
                None
            }
        }
    }

    pub fn get_observations(&self) -> Vec<Observation> {
        self.observations.lock().unwrap().clone()
    }
}
