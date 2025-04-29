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
// Use std::sync::Mutex for observations, as it's mostly accessed synchronously
use chrono::{DateTime, Utc};
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn}; // Added for timestamping feedback

// --- Agent Definition ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    prev_state: Option<String>,
    event: String,
    current_state: String,
    // Consider adding context snapshot here for richer history
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackOutcome {
    SuccessStateChanged,
    SuccessNoStateChange,
    InvalidEventForState, // If send() rejects it based on current state
    SendError(String),    // If send() returns Err
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feedback {
    pub timestamp: DateTime<Utc>,
    pub attempted_event: Option<TodoEvent>,
    pub outcome: FeedbackOutcome,
    pub reason: Option<String>,
    pub goal_at_time: String,
    pub state_at_time: String,
}

#[derive(Debug)]
pub struct Agent {
    name: String,
    client: OpenAiClient<OpenAIConfig>,
    observations: Arc<Mutex<Vec<Observation>>>, // Keep observations thread-safe
    feedback: Arc<Mutex<Vec<Feedback>>>,        // Added feedback field
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
            feedback: Arc::new(Mutex::new(Vec::new())), // Initialize feedback
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

    // Method to add feedback
    pub fn add_feedback(&self, feedback: Feedback) {
        info!("Agent '{}' received feedback: {:?}", self.name, feedback);
        let mut feedbacks = self.feedback.lock().unwrap();
        feedbacks.push(feedback);
        // Optional: Limit feedback history size
        // const MAX_FEEDBACK: usize = 10;
        // if feedbacks.len() > MAX_FEEDBACK {
        //     feedbacks.remove(0);
        // }
    }

    pub async fn decide(
        &self,
        current_state: &TodoState,
        context_arc: &Arc<tokio::sync::RwLock<TodoContext>>,
        goal: &str,
    ) -> Option<TodoEvent> {
        let state_name = current_state.name();
        info!(
            "Agent '{}' deciding based on goal: '{}'. Current state: {}.",
            self.name, goal, state_name,
        );

        // Lock context internally
        let context_guard = context_arc.read().await;
        let context_json = serde_json::to_string_pretty(&*context_guard)
            .unwrap_or_else(|_| "Context unavailable".to_string());
        drop(context_guard); // Drop guard after reading context

        // Lock observations and feedback
        let observations_json;
        let feedback_json;
        {
            // Scope for mutex guards
            let obs_guard = self.observations.lock().unwrap();
            observations_json = serde_json::to_string_pretty(&*obs_guard)
                .unwrap_or_else(|_| "Observations unavailable".to_string());

            let feedback_guard = self.feedback.lock().unwrap();
            // Get recent feedback (e.g., last 5)
            let recent_feedback: Vec<_> = feedback_guard.iter().rev().take(5).cloned().collect();
            feedback_json =
                serde_json::to_string_pretty(&recent_feedback.iter().rev().collect::<Vec<_>>()) // Reverse back for chronological order in prompt
                    .unwrap_or_else(|_| "Feedback unavailable".to_string());
        } // Guards dropped here

        let event_schema_description = match current_state {
            TodoState::Idle => {
                r#"
- {"type": "Add", "title": "string", "content": "string"}
- {"type": "View"}
(Added, Viewed, BackToIdle イベントはシステムが発行するため、生成しないでください)
"#
            }
            // Add descriptions for other states if they allow specific events
            _ => "(この状態からユーザー/エージェントが直接発行できるイベントはありません)",
        };

        // Updated system prompt with feedback
        let system_prompt = format!(
            r#"
あなたは効率的なタスクマネージャーとして動作するAIエージェントであり、Todoリストの状態機械を制御します。
あなたの現在の目標は: {}

# 状態とコンテキスト
現在の状態: {}
現在のコンテキスト (Todoリスト):
{}

# 観測履歴 (最近の遷移)
{}

# 最近のフィードバック (あなたの試行とその結果)
{}

# あなたのタスク
上記の目標、現在の状態、コンテキスト、観測履歴、そして**特に最近のフィードバック**を考慮し、目標達成に向けて状態機械に送信すべき**次の単一のイベント**を決定してください。失敗した試行を繰り返さないように注意してください。

# 制約事項
- **現在の状態で許可されているイベントのみ**を出力しなければなりません。
- 出力は、以下の「許可されるイベントスキーマ」に厳密に従うJSONオブジェクト、**または**、現時点で適切なアクションがない場合は文字列 "NO_ACTION" の**いずれかのみ**です。他のテキストは一切含めないでください。
- 観測履歴とフィードバックを参考に、目標達成に最も効果的なアクションを選択してください。

# 許可されるイベントスキーマ (現在の状態: {})
{}

# 出力:
(ここにJSONイベントオブジェクトまたは"NO_ACTION"のみを出力)

"#,
            goal,
            state_name,
            context_json,
            observations_json,
            feedback_json, // Added feedback here
            state_name,    // For schema title
            event_schema_description
        );

        // User message can be simplified or adjusted
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
                if let Some(choice) = response.choices.first() {
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
