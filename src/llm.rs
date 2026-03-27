use std::fmt;
use std::io::Error as IoError;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Result, anyhow};
use rig::client::{CompletionClient, Nothing, ProviderClient};
use rig::completion::{Prompt, ToolDefinition};
use rig::providers::{ollama, openai};
use rig::tool::{Tool, ToolDyn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::time::{Duration, timeout};
use tracing::{debug, info, trace, warn};

use crate::ai::context::ActorTurnContext;
use crate::ai::prompting::build_turn_prompt;
use crate::domain::commands::{ActionKind, AgentAvailableAction};
use crate::domain::events::{DialogueLine, DialogueSpeaker};
use crate::domain::memory::ConversationMemory;
use crate::world::{ActorId, EntityId, PlaceId, place_name_from_parts};

const SPEAK_TO_TOOL_NAME: &str = "speak_to";
const MOVE_TO_TOOL_NAME: &str = "move_to";
const INSPECT_ENTITY_TOOL_NAME: &str = "inspect_entity";
const DO_NOTHING_TOOL_NAME: &str = "do_nothing";

#[allow(async_fn_in_trait)]
pub trait LlmBackend {
    async fn choose_action(&self, context: &ActorTurnContext) -> Result<ActionSelection>;

    async fn summarize_memory(&self, transcript: &[DialogueLine]) -> Result<ConversationMemory>;

    fn name(&self) -> &'static str;

    fn label(&self) -> String {
        self.name().to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionSelection {
    pub action: ActionKind,
    pub trace: AgentDebugTrace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentDebugTrace {
    pub actor_id: ActorId,
    pub backend_name: String,
    pub prompt: String,
    pub toolset: Vec<String>,
    pub available_actions: Vec<AgentAvailableAction>,
    pub recent_speech: Vec<DialogueLine>,
    pub tool_calls: Vec<AgentToolTrace>,
    pub model_output: Option<String>,
    pub selected_action: Option<ActionKind>,
    pub error: Option<String>,
}

impl AgentDebugTrace {
    pub fn from_context(context: &ActorTurnContext, backend_name: &str) -> Self {
        Self {
            actor_id: context.actor.id,
            backend_name: backend_name.to_string(),
            prompt: build_turn_prompt(context),
            toolset: Vec::new(),
            available_actions: context.available_actions.clone(),
            recent_speech: context.recent_speech.clone(),
            tool_calls: Vec::new(),
            model_output: None,
            selected_action: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolTrace {
    pub tool_name: String,
    pub arguments: String,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AnyBackend {
    Mock(MockBackend),
    Rig(RigBackend),
}

impl AnyBackend {
    pub fn from_env() -> Result<Self> {
        if std::env::var("OLLAMA_MODEL").is_ok()
            || std::env::var("OPENAI_API_KEY").is_ok()
            || std::env::var("OPENAI_BASE_URL").is_ok()
        {
            Ok(Self::Rig(RigBackend::from_env()?))
        } else {
            Ok(Self::Mock(MockBackend))
        }
    }
}

impl LlmBackend for AnyBackend {
    async fn choose_action(&self, context: &ActorTurnContext) -> Result<ActionSelection> {
        match self {
            Self::Mock(backend) => backend.choose_action(context).await,
            Self::Rig(backend) => backend.choose_action(context).await,
        }
    }

    async fn summarize_memory(&self, transcript: &[DialogueLine]) -> Result<ConversationMemory> {
        match self {
            Self::Mock(backend) => backend.summarize_memory(transcript).await,
            Self::Rig(backend) => backend.summarize_memory(transcript).await,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Mock(backend) => backend.name(),
            Self::Rig(backend) => backend.name(),
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Mock(backend) => backend.label(),
            Self::Rig(backend) => backend.label(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MockBackend;

impl LlmBackend for MockBackend {
    async fn choose_action(&self, context: &ActorTurnContext) -> Result<ActionSelection> {
        debug!(
            actor_id = context.actor.id.index(),
            backend = self.label(),
            "mock backend choosing action"
        );
        let mut trace = AgentDebugTrace::from_context(context, self.name());
        trace.toolset = available_tool_names(context);

        let action = if let Some((speaker_id, speaker_input)) = latest_inbound_speech(context)
            && context
                .available_actions
                .contains(&AgentAvailableAction::SpeakTo { target: speaker_id })
        {
            trace.model_output =
                Some("mock policy: replying to the latest inbound speech".to_string());
            ActionKind::Speak {
                target: speaker_id,
                text: mock_reply_text(context, &speaker_input),
            }
        } else if context.actor.id.index() % 4 == 0 {
            if let Some(destination) = first_move_destination(context) {
                trace.model_output =
                    Some("mock policy: moving first when this actor tends to roam".to_string());
                ActionKind::MoveTo { destination }
            } else if let Some(entity_id) = first_nearby_entity(context) {
                trace.model_output =
                    Some("mock policy: inspecting because there is something nearby".to_string());
                ActionKind::InspectEntity { entity_id }
            } else if let Some(target) = preferred_speak_target(context) {
                trace.model_output =
                    Some("mock policy: proactively starting a conversation".to_string());
                ActionKind::Speak {
                    target,
                    text: mock_initiation_text(context, target),
                }
            } else {
                trace.model_output =
                    Some("mock policy: nothing useful to do, staying idle".to_string());
                ActionKind::DoNothing
            }
        } else if context.actor.id.index() % 4 == 1 {
            if let Some(entity_id) = first_nearby_entity(context) {
                trace.model_output =
                    Some("mock policy: inspecting the most obvious nearby entity".to_string());
                ActionKind::InspectEntity { entity_id }
            } else if let Some(target) = preferred_speak_target(context) {
                trace.model_output =
                    Some("mock policy: proactively starting a conversation".to_string());
                ActionKind::Speak {
                    target,
                    text: mock_initiation_text(context, target),
                }
            } else if let Some(destination) = first_move_destination(context) {
                trace.model_output =
                    Some("mock policy: moving because nothing else stands out".to_string());
                ActionKind::MoveTo { destination }
            } else {
                trace.model_output =
                    Some("mock policy: nothing useful to do, staying idle".to_string());
                ActionKind::DoNothing
            }
        } else if let Some(target) = preferred_speak_target(context) {
            trace.model_output =
                Some("mock policy: proactively starting a conversation".to_string());
            ActionKind::Speak {
                target,
                text: mock_initiation_text(context, target),
            }
        } else if let Some(destination) = first_move_destination(context) {
            trace.model_output = Some("mock policy: moving to another room".to_string());
            ActionKind::MoveTo { destination }
        } else if let Some(entity_id) = first_nearby_entity(context) {
            trace.model_output = Some("mock policy: inspecting a nearby entity".to_string());
            ActionKind::InspectEntity { entity_id }
        } else {
            trace.model_output =
                Some("mock policy: nothing useful to do, staying idle".to_string());
            ActionKind::DoNothing
        };
        trace.selected_action = Some(action.clone());
        Ok(ActionSelection { action, trace })
    }

    async fn summarize_memory(&self, transcript: &[DialogueLine]) -> Result<ConversationMemory> {
        Ok(ConversationMemory {
            summary: fallback_summary(transcript),
        }
        .normalized())
    }

    fn name(&self) -> &'static str {
        "mock"
    }

    fn label(&self) -> String {
        "mock".to_string()
    }
}

#[derive(Clone)]
pub struct RigBackend {
    provider: RigProvider,
}

#[derive(Clone)]
enum RigProvider {
    Ollama {
        client: ollama::Client,
        model: String,
    },
    OpenAiCompatible {
        client: openai::Client,
        model: String,
    },
}

impl fmt::Debug for RigBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.provider {
            RigProvider::Ollama { model, .. } => f
                .debug_struct("RigBackend")
                .field("provider", &"ollama")
                .field("model", model)
                .finish(),
            RigProvider::OpenAiCompatible { model, .. } => f
                .debug_struct("RigBackend")
                .field("provider", &"openai-compatible")
                .field("model", model)
                .finish(),
        }
    }
}

impl RigBackend {
    fn label(&self) -> String {
        match &self.provider {
            RigProvider::Ollama { model, .. } => format!("rig/ollama ({model})"),
            RigProvider::OpenAiCompatible { model, .. } => {
                format!("rig/openai-compatible ({model})")
            }
        }
    }

    pub fn from_env() -> Result<Self> {
        if let Ok(model) = std::env::var("OLLAMA_MODEL") {
            let client = if std::env::var("OLLAMA_API_BASE_URL").is_ok() {
                ollama::Client::from_env()
            } else {
                ollama::Client::new(Nothing)?
            };
            return Ok(Self {
                provider: RigProvider::Ollama { client, model },
            });
        }

        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let client = openai::Client::from_env();
        Ok(Self {
            provider: RigProvider::OpenAiCompatible { client, model },
        })
    }

    async fn tool_choose_action(&self, context: &ActorTurnContext) -> Result<ActionSelection> {
        let prompt = build_turn_prompt(context);
        let tool_names = available_tool_names(context);
        debug!(
            actor_id = context.actor.id.index(),
            backend = %self.label(),
            prompt_length = prompt.len(),
            toolset = ?tool_names,
            available_actions = ?context.available_actions,
            recent_speech_count = context.recent_speech.len(),
            "starting rig autonomous tool action selection"
        );
        trace!(
            actor_id = context.actor.id.index(),
            backend = %self.label(),
            prompt = %prompt,
            available_actions = ?context.available_actions,
            recent_speech = ?context.recent_speech,
            "autonomous turn prompt"
        );
        let selected_action = Arc::new(Mutex::new(None));
        let tool_calls = Arc::new(Mutex::new(Vec::new()));
        let mut tools: Vec<Box<dyn ToolDyn>> = Vec::new();
        let speak_choices = speak_choices(context);
        let move_choices = move_choices(context);
        let inspect_choices = inspect_choices(context);

        if !speak_choices.is_empty() {
            tools.push(Box::new(SpeakToTool::new(
                context.actor.id,
                speak_choices.clone(),
                Arc::clone(&selected_action),
                Arc::clone(&tool_calls),
            )));
        }
        if !move_choices.is_empty() {
            tools.push(Box::new(MoveToTool::new(
                context.actor.id,
                move_choices.clone(),
                Arc::clone(&selected_action),
                Arc::clone(&tool_calls),
            )));
        }
        if !inspect_choices.is_empty() {
            tools.push(Box::new(InspectEntityTool::new(
                context.actor.id,
                inspect_choices.clone(),
                Arc::clone(&selected_action),
                Arc::clone(&tool_calls),
            )));
        }
        tools.push(Box::new(DoNothingTool::new(
            context.actor.id,
            Arc::clone(&selected_action),
            Arc::clone(&tool_calls),
        )));

        debug!(
            actor_id = context.actor.id.index(),
            backend = %self.label(),
            toolset = ?tool_names,
            speak_choices = ?speak_choices.iter().map(|choice| choice.id.index()).collect::<Vec<_>>(),
            move_choices = ?move_choices.iter().map(|choice| choice.id.index()).collect::<Vec<_>>(),
            inspect_choices = ?inspect_choices.iter().map(|choice| choice.id.index()).collect::<Vec<_>>(),
            "autonomous tools prepared"
        );
        let request_started = Instant::now();
        let request_future = async {
            match &self.provider {
                RigProvider::Ollama { client, model } => {
                    debug!(
                        actor_id = context.actor.id.index(),
                        backend = %self.label(),
                        model,
                        timeout_seconds = TURN_SELECTION_TIMEOUT.as_secs(),
                        "dispatching ollama autonomous agent request"
                    );
                    let agent = client
                        .agent(model.clone())
                        .preamble(TURN_PREAMBLE)
                        .temperature(0.0)
                        .max_tokens(TURN_MAX_TOKENS)
                        .additional_params(json!({ "think": false }))
                        .tools(tools)
                        .build();
                    agent.prompt(prompt.clone()).max_turns(1).await
                }
                RigProvider::OpenAiCompatible { client, model } => {
                    debug!(
                        actor_id = context.actor.id.index(),
                        backend = %self.label(),
                        model,
                        timeout_seconds = TURN_SELECTION_TIMEOUT.as_secs(),
                        "dispatching openai-compatible autonomous agent request"
                    );
                    let agent = client
                        .agent(model.clone())
                        .preamble(TURN_PREAMBLE)
                        .temperature(0.0)
                        .max_tokens(TURN_MAX_TOKENS)
                        .tools(tools)
                        .build();
                    agent.prompt(prompt.clone()).max_turns(1).await
                }
            }
        };
        tokio::pin!(request_future);

        let mut completion = None;
        let mut timed_out = false;
        let mut cancelled_after_commit = false;
        let mut selected_seen_at: Option<Instant> = None;

        loop {
            if request_started.elapsed() >= TURN_SELECTION_TIMEOUT {
                timed_out = true;
                break;
            }

            tokio::select! {
                result = &mut request_future => {
                    completion = Some(result);
                    break;
                }
                _ = tokio::time::sleep(TURN_POLL_INTERVAL) => {
                    let has_selected = selected_action
                        .lock()
                        .map_err(|_| anyhow!("autonomous action selection lock poisoned"))?
                        .is_some();
                    if has_selected {
                        let selected_since = selected_seen_at.get_or_insert_with(Instant::now);
                        debug!(
                            actor_id = context.actor.id.index(),
                            backend = %self.label(),
                            since_commit_ms = selected_since.elapsed().as_millis(),
                            elapsed_ms = request_started.elapsed().as_millis(),
                            "autonomous action committed while provider request is still running"
                        );
                        if selected_since.elapsed() >= TURN_POST_COMMIT_GRACE {
                            cancelled_after_commit = true;
                            info!(
                                actor_id = context.actor.id.index(),
                                backend = %self.label(),
                                since_commit_ms = selected_since.elapsed().as_millis(),
                                elapsed_ms = request_started.elapsed().as_millis(),
                                "ending autonomous request after committed-action grace period"
                            );
                            break;
                        }
                    }
                }
            }
        }
        let request_elapsed = request_started.elapsed();

        let selected = selected_action
            .lock()
            .map_err(|_| anyhow!("autonomous action selection lock poisoned"))?
            .clone();
        let action = selected.clone().unwrap_or(ActionKind::DoNothing);
        let mut trace = AgentDebugTrace::from_context(context, &self.label());
        trace.prompt = prompt.clone();
        trace.toolset = tool_names.clone();
        trace.tool_calls = tool_calls
            .lock()
            .map_err(|_| anyhow!("tool call trace lock poisoned"))?
            .clone();
        debug!(
            actor_id = context.actor.id.index(),
            backend = %self.label(),
            elapsed_ms = request_elapsed.as_millis(),
            selected_action_present = selected.is_some(),
            tool_call_count = trace.tool_calls.len(),
            "autonomous agent request completed"
        );

        match completion {
            Some(Ok(model_output)) => {
                debug!(
                    actor_id = context.actor.id.index(),
                    backend = %self.label(),
                    elapsed_ms = request_elapsed.as_millis(),
                    model_output_len = model_output.len(),
                    "autonomous agent returned a completion payload"
                );
                trace!(
                    actor_id = context.actor.id.index(),
                    backend = %self.label(),
                    model_output = %model_output,
                    "autonomous agent raw completion"
                );
                trace.model_output = Some(model_output);
            }
            Some(Err(error)) => {
                trace.error = Some(error.to_string());
                if selected.is_some() {
                    warn!(
                        actor_id = context.actor.id.index(),
                        backend = %self.label(),
                        error = %error,
                        selected_action = ?action,
                        elapsed_ms = request_elapsed.as_millis(),
                        "autonomous agent returned an error after committing an action; accepting the committed action"
                    );
                } else {
                    warn!(
                        actor_id = context.actor.id.index(),
                        backend = %self.label(),
                        error = %error,
                        elapsed_ms = request_elapsed.as_millis(),
                        "autonomous agent failed without a committed action"
                    );
                }
            }
            None if timed_out => {
                let timeout_message = format!(
                    "autonomous selection timed out after {}s",
                    TURN_SELECTION_TIMEOUT.as_secs()
                );
                trace.error = Some(timeout_message);
                if selected.is_some() {
                    warn!(
                        actor_id = context.actor.id.index(),
                        backend = %self.label(),
                        selected_action = ?action,
                        timeout_seconds = TURN_SELECTION_TIMEOUT.as_secs(),
                        elapsed_ms = request_elapsed.as_millis(),
                        "autonomous agent timed out after committing an action; accepting the committed action"
                    );
                } else {
                    warn!(
                        actor_id = context.actor.id.index(),
                        backend = %self.label(),
                        timeout_seconds = TURN_SELECTION_TIMEOUT.as_secs(),
                        elapsed_ms = request_elapsed.as_millis(),
                        "autonomous agent timed out without a committed action"
                    );
                }
            }
            None if cancelled_after_commit => {
                info!(
                    actor_id = context.actor.id.index(),
                    backend = %self.label(),
                    elapsed_ms = request_elapsed.as_millis(),
                    tool_call_count = trace.tool_calls.len(),
                    "autonomous request ended after committed-action grace period"
                );
            }
            None => unreachable!(
                "autonomous request should either complete, time out, or cancel after commit"
            ),
        }

        if selected.is_none() && matches!(action, ActionKind::DoNothing) {
            let prior_error = trace.error.take();
            trace.error = Some(match prior_error {
                Some(error) => format!("{error}; no tool committed an action"),
                None => "no tool committed an action".to_string(),
            });
            warn!(
                actor_id = context.actor.id.index(),
                backend = %self.label(),
                elapsed_ms = request_elapsed.as_millis(),
                model_output_present = trace.model_output.is_some(),
                tool_call_count = trace.tool_calls.len(),
                "autonomous agent completed without a committed tool action"
            );
        }

        trace.selected_action = Some(action.clone());
        info!(
            actor_id = context.actor.id.index(),
            backend = %self.label(),
            selected_action = ?action,
            tool_call_count = trace.tool_calls.len(),
            elapsed_ms = request_elapsed.as_millis(),
            "rig autonomous action selection finished"
        );
        trace!(
            actor_id = context.actor.id.index(),
            backend = %self.label(),
            tool_calls = ?trace.tool_calls,
            "rig autonomous tool trace"
        );

        Ok(ActionSelection { action, trace })
    }

    async fn agent_choose_action(&self, context: &ActorTurnContext) -> Result<ActionSelection> {
        debug!(
            actor_id = context.actor.id.index(),
            backend = %self.label(),
            "entering generic autonomous tool path"
        );
        self.tool_choose_action(context).await
    }

    async fn prompt_memory_summary_text(&self, transcript: &str) -> Result<String> {
        let prompt = format!(
            "Summarize what these two actors talked about in 1-2 durable sentences.\n\nConversation:\n{}",
            transcript
        );

        match &self.provider {
            RigProvider::Ollama { client, model } => {
                let agent = client
                    .agent(model.clone())
                    .preamble(MEMORY_PREAMBLE)
                    .temperature(0.2)
                    .max_tokens(MEMORY_MAX_TOKENS)
                    .additional_params(json!({ "think": false }))
                    .build();
                Ok(agent.prompt(prompt).await?)
            }
            RigProvider::OpenAiCompatible { client, model } => {
                let agent = client
                    .agent(model.clone())
                    .preamble(MEMORY_PREAMBLE)
                    .temperature(0.2)
                    .max_tokens(MEMORY_MAX_TOKENS)
                    .build();
                Ok(agent.prompt(prompt).await?)
            }
        }
    }
}

impl LlmBackend for RigBackend {
    async fn choose_action(&self, context: &ActorTurnContext) -> Result<ActionSelection> {
        self.agent_choose_action(context).await
    }

    async fn summarize_memory(&self, transcript: &[DialogueLine]) -> Result<ConversationMemory> {
        if transcript.len() <= 8 {
            return Ok(ConversationMemory {
                summary: fallback_summary(transcript),
            }
            .normalized());
        }

        let transcript_text = transcript
            .iter()
            .map(|line| format!("{}: {}", speaker_label(line), line.text))
            .collect::<Vec<_>>()
            .join("\n");

        let summary = match timeout(
            MEMORY_SUMMARY_TIMEOUT,
            self.prompt_memory_summary_text(&transcript_text),
        )
        .await
        {
            Ok(Ok(summary)) => summary,
            Ok(Err(error)) => {
                warn!(
                    backend = %self.label(),
                    error = %error,
                    "memory summarization failed; using fallback summary"
                );
                fallback_summary(transcript)
            }
            Err(_) => {
                warn!(
                    backend = %self.label(),
                    timeout_seconds = MEMORY_SUMMARY_TIMEOUT.as_secs(),
                    "memory summarization timed out; using fallback summary"
                );
                fallback_summary(transcript)
            }
        };

        Ok(ConversationMemory { summary }.normalized())
    }

    fn name(&self) -> &'static str {
        match self.provider {
            RigProvider::Ollama { .. } => "rig/ollama",
            RigProvider::OpenAiCompatible { .. } => "rig/openai-compatible",
        }
    }

    fn label(&self) -> String {
        RigBackend::label(self)
    }
}

#[derive(Debug, Clone, Serialize)]
struct ActionAcknowledgement {
    accepted: bool,
    action: ActionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpeakToArgs {
    target_actor_id: usize,
    text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MoveToArgs {
    destination_place_id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InspectEntityArgs {
    entity_id: usize,
}

#[derive(Debug, Clone)]
struct ActorTargetChoice {
    id: ActorId,
    label: String,
}

#[derive(Debug, Clone)]
struct PlaceChoice {
    id: PlaceId,
    label: String,
}

#[derive(Debug, Clone)]
struct EntityChoice {
    id: EntityId,
    label: String,
}

type SelectedActionSink = Arc<Mutex<Option<ActionKind>>>;
type ToolTraceSink = Arc<Mutex<Vec<AgentToolTrace>>>;

struct SpeakToTool {
    actor_id: ActorId,
    choices: Vec<ActorTargetChoice>,
    selected_action: SelectedActionSink,
    tool_calls: ToolTraceSink,
}

impl SpeakToTool {
    fn new(
        actor_id: ActorId,
        choices: Vec<ActorTargetChoice>,
        selected_action: SelectedActionSink,
        tool_calls: ToolTraceSink,
    ) -> Self {
        Self {
            actor_id,
            choices,
            selected_action,
            tool_calls,
        }
    }
}

impl Tool for SpeakToTool {
    const NAME: &'static str = SPEAK_TO_TOOL_NAME;
    type Error = IoError;
    type Args = SpeakToArgs;
    type Output = ActionAcknowledgement;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!(
                "Speak to a nearby actor. Valid targets: {}. Provide only the exact in-character words to say.",
                render_actor_choices(&self.choices)
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "target_actor_id": {
                        "type": "integer",
                        "description": format!(
                            "Nearby actor id. Choose one of: {}",
                            self.choices.iter().map(|choice| choice.id.index().to_string()).collect::<Vec<_>>().join(", ")
                        )
                    },
                    "text": {
                        "type": "string",
                        "description": "The exact short line to say, 1-3 short sentences, no narration or speaker label."
                    }
                },
                "required": ["target_actor_id", "text"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        debug!(
            tool_name = Self::NAME,
            actor_id = self.actor_id.index(),
            target_actor_id = args.target_actor_id,
            text_len = args.text.len(),
            "speak_to invoked"
        );
        let Some(target_choice) = self
            .choices
            .iter()
            .find(|choice| choice.id.index() == args.target_actor_id)
        else {
            let error = format!(
                "invalid target_actor_id {}; valid targets are {}",
                args.target_actor_id,
                render_actor_choices(&self.choices)
            );
            push_tool_trace(
                &self.tool_calls,
                Self::NAME,
                &args,
                None,
                Some(error.clone()),
            );
            return Err(IoError::other(error));
        };

        let text = args.text.trim();
        if text.is_empty() {
            let error = "speak_to requires non-empty text".to_string();
            push_tool_trace(
                &self.tool_calls,
                Self::NAME,
                &args,
                None,
                Some(error.clone()),
            );
            return Err(IoError::other(error));
        }

        let action = ActionKind::Speak {
            target: target_choice.id,
            text: text.to_string(),
        };
        commit_action(
            &self.selected_action,
            &self.tool_calls,
            Self::NAME,
            &args,
            action,
        )
    }
}

struct MoveToTool {
    actor_id: ActorId,
    choices: Vec<PlaceChoice>,
    selected_action: SelectedActionSink,
    tool_calls: ToolTraceSink,
}

impl MoveToTool {
    fn new(
        actor_id: ActorId,
        choices: Vec<PlaceChoice>,
        selected_action: SelectedActionSink,
        tool_calls: ToolTraceSink,
    ) -> Self {
        Self {
            actor_id,
            choices,
            selected_action,
            tool_calls,
        }
    }
}

impl Tool for MoveToTool {
    const NAME: &'static str = MOVE_TO_TOOL_NAME;
    type Error = IoError;
    type Args = MoveToArgs;
    type Output = ActionAcknowledgement;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!(
                "Move to an adjacent room. Valid destinations: {}.",
                render_place_choices(&self.choices)
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "destination_place_id": {
                        "type": "integer",
                        "description": format!(
                            "Adjacent room id. Choose one of: {}",
                            self.choices.iter().map(|choice| choice.id.index().to_string()).collect::<Vec<_>>().join(", ")
                        )
                    }
                },
                "required": ["destination_place_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        debug!(
            tool_name = Self::NAME,
            actor_id = self.actor_id.index(),
            destination_place_id = args.destination_place_id,
            "move_to invoked"
        );
        let Some(destination_choice) = self
            .choices
            .iter()
            .find(|choice| choice.id.index() == args.destination_place_id)
        else {
            let error = format!(
                "invalid destination_place_id {}; valid destinations are {}",
                args.destination_place_id,
                render_place_choices(&self.choices)
            );
            push_tool_trace(
                &self.tool_calls,
                Self::NAME,
                &args,
                None,
                Some(error.clone()),
            );
            return Err(IoError::other(error));
        };

        let action = ActionKind::MoveTo {
            destination: destination_choice.id,
        };
        commit_action(
            &self.selected_action,
            &self.tool_calls,
            Self::NAME,
            &args,
            action,
        )
    }
}

struct InspectEntityTool {
    actor_id: ActorId,
    choices: Vec<EntityChoice>,
    selected_action: SelectedActionSink,
    tool_calls: ToolTraceSink,
}

impl InspectEntityTool {
    fn new(
        actor_id: ActorId,
        choices: Vec<EntityChoice>,
        selected_action: SelectedActionSink,
        tool_calls: ToolTraceSink,
    ) -> Self {
        Self {
            actor_id,
            choices,
            selected_action,
            tool_calls,
        }
    }
}

impl Tool for InspectEntityTool {
    const NAME: &'static str = INSPECT_ENTITY_TOOL_NAME;
    type Error = IoError;
    type Args = InspectEntityArgs;
    type Output = ActionAcknowledgement;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: format!(
                "Inspect a nearby entity. Valid entities: {}.",
                render_entity_choices(&self.choices)
            ),
            parameters: json!({
                "type": "object",
                "properties": {
                    "entity_id": {
                        "type": "integer",
                        "description": format!(
                            "Nearby entity id. Choose one of: {}",
                            self.choices.iter().map(|choice| choice.id.index().to_string()).collect::<Vec<_>>().join(", ")
                        )
                    }
                },
                "required": ["entity_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        debug!(
            tool_name = Self::NAME,
            actor_id = self.actor_id.index(),
            entity_id = args.entity_id,
            "inspect_entity invoked"
        );
        let Some(entity_choice) = self
            .choices
            .iter()
            .find(|choice| choice.id.index() == args.entity_id)
        else {
            let error = format!(
                "invalid entity_id {}; valid entities are {}",
                args.entity_id,
                render_entity_choices(&self.choices)
            );
            push_tool_trace(
                &self.tool_calls,
                Self::NAME,
                &args,
                None,
                Some(error.clone()),
            );
            return Err(IoError::other(error));
        };

        let action = ActionKind::InspectEntity {
            entity_id: entity_choice.id,
        };
        commit_action(
            &self.selected_action,
            &self.tool_calls,
            Self::NAME,
            &args,
            action,
        )
    }
}

struct DoNothingTool {
    actor_id: ActorId,
    selected_action: SelectedActionSink,
    tool_calls: ToolTraceSink,
}

impl DoNothingTool {
    fn new(
        actor_id: ActorId,
        selected_action: SelectedActionSink,
        tool_calls: ToolTraceSink,
    ) -> Self {
        Self {
            actor_id,
            selected_action,
            tool_calls,
        }
    }
}

impl Tool for DoNothingTool {
    const NAME: &'static str = DO_NOTHING_TOOL_NAME;
    type Error = IoError;
    type Args = serde_json::Value;
    type Output = ActionAcknowledgement;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Decline to act right now. Use this if no other available action is worth taking."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        debug!(
            tool_name = Self::NAME,
            actor_id = self.actor_id.index(),
            "do_nothing invoked"
        );
        commit_action(
            &self.selected_action,
            &self.tool_calls,
            Self::NAME,
            &args,
            ActionKind::DoNothing,
        )
    }
}

fn serialize_pretty<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value)
        .unwrap_or_else(|error| format!("{{\"serialization_error\":\"{}\"}}", error))
}

fn push_tool_trace(
    sink: &ToolTraceSink,
    tool_name: &str,
    args: &impl Serialize,
    result: Option<String>,
    error: Option<String>,
) {
    let serialized_args = serialize_pretty(args);
    if let Some(error) = &error {
        warn!(tool_name, arguments = %serialized_args, error = %error, "tool call failed");
    } else {
        debug!(
            tool_name,
            arguments = %serialized_args,
            result = %result.as_deref().unwrap_or("<none>"),
            "tool call completed"
        );
    }
    if let Ok(mut sink) = sink.lock() {
        sink.push(AgentToolTrace {
            tool_name: tool_name.to_string(),
            arguments: serialized_args,
            result,
            error,
        });
    }
}

fn commit_action(
    selected_action: &SelectedActionSink,
    tool_calls: &ToolTraceSink,
    tool_name: &str,
    args: &impl Serialize,
    action: ActionKind,
) -> Result<ActionAcknowledgement, IoError> {
    let mut guard = selected_action
        .lock()
        .map_err(|_| IoError::other("selected action lock poisoned"))?;
    if guard.is_some() {
        let error = "an action was already selected".to_string();
        push_tool_trace(tool_calls, tool_name, args, None, Some(error.clone()));
        return Err(IoError::other(error));
    }

    *guard = Some(action.clone());
    debug!(tool_name, action = ?action, "tool committed action");

    let output = ActionAcknowledgement {
        accepted: true,
        action,
    };
    push_tool_trace(
        tool_calls,
        tool_name,
        args,
        Some(serialize_pretty(&output)),
        None,
    );
    Ok(output)
}

fn available_tool_names(context: &ActorTurnContext) -> Vec<String> {
    let mut tool_names = Vec::new();
    if context
        .available_actions
        .iter()
        .any(|action| matches!(action, AgentAvailableAction::SpeakTo { .. }))
    {
        tool_names.push(SPEAK_TO_TOOL_NAME.to_string());
    }
    if context
        .available_actions
        .iter()
        .any(|action| matches!(action, AgentAvailableAction::MoveTo { .. }))
    {
        tool_names.push(MOVE_TO_TOOL_NAME.to_string());
    }
    if context
        .available_actions
        .iter()
        .any(|action| matches!(action, AgentAvailableAction::InspectEntity { .. }))
    {
        tool_names.push(INSPECT_ENTITY_TOOL_NAME.to_string());
    }
    tool_names.push(DO_NOTHING_TOOL_NAME.to_string());
    tool_names
}

fn latest_inbound_speech(context: &ActorTurnContext) -> Option<(ActorId, String)> {
    context
        .recent_speech
        .iter()
        .rev()
        .next()
        .and_then(|line| match line.speaker {
            DialogueSpeaker::Actor(actor_id) if actor_id != context.actor.id => {
                Some((actor_id, line.text.clone()))
            }
            _ => None,
        })
}

fn speak_choices(context: &ActorTurnContext) -> Vec<ActorTargetChoice> {
    let mut choices = context
        .available_actions
        .iter()
        .filter_map(|action| match action {
            AgentAvailableAction::SpeakTo { target } => Some(*target),
            _ => None,
        })
        .map(|id| ActorTargetChoice {
            id,
            label: context
                .local_state
                .nearby_actors
                .iter()
                .find(|actor| actor.id == id)
                .map(|actor| {
                    format!(
                        "{} ({}, {})",
                        actor.name(context.world_seed),
                        actor.occupation.label(),
                        actor.archetype.label()
                    )
                })
                .unwrap_or_else(|| id.name(context.world_seed)),
        })
        .collect::<Vec<_>>();
    choices.sort_by_key(|choice| choice.id.index());
    choices
}

fn move_choices(context: &ActorTurnContext) -> Vec<PlaceChoice> {
    let mut choices = context
        .available_actions
        .iter()
        .filter_map(|action| match action {
            AgentAvailableAction::MoveTo { destination } => Some(*destination),
            _ => None,
        })
        .map(|id| {
            let label = context
                .local_state
                .routes
                .iter()
                .find(|route| route.destination.id == id)
                .map(|route| {
                    format!(
                        "{} via {}s",
                        place_name_from_parts(
                            context.world_seed,
                            route.destination.id,
                            route.destination.city_id,
                            route.destination.kind,
                        ),
                        route.travel_time.seconds()
                    )
                })
                .unwrap_or_else(|| {
                    format!(
                        "{}",
                        place_name_from_parts(
                            context.world_seed,
                            id,
                            context.current_place.city_id,
                            context.current_place.kind,
                        )
                    )
                });
            PlaceChoice { id, label }
        })
        .collect::<Vec<_>>();
    choices.sort_by_key(|choice| choice.id.index());
    choices
}

fn inspect_choices(context: &ActorTurnContext) -> Vec<EntityChoice> {
    let mut choices = context
        .available_actions
        .iter()
        .filter_map(|action| match action {
            AgentAvailableAction::InspectEntity { entity_id } => Some(*entity_id),
            _ => None,
        })
        .map(|id| {
            let label = context
                .local_state
                .nearby_entities
                .iter()
                .find(|entity| entity.id == id)
                .map(|entity| entity.kind.label().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            EntityChoice { id, label }
        })
        .collect::<Vec<_>>();
    choices.sort_by_key(|choice| choice.id.index());
    choices
}

fn render_actor_choices(choices: &[ActorTargetChoice]) -> String {
    if choices.is_empty() {
        "none".to_string()
    } else {
        choices
            .iter()
            .map(|choice| format!("actor#{} {}", choice.id.index(), choice.label))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn render_place_choices(choices: &[PlaceChoice]) -> String {
    if choices.is_empty() {
        "none".to_string()
    } else {
        choices
            .iter()
            .map(|choice| format!("place#{} {}", choice.id.index(), choice.label))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn render_entity_choices(choices: &[EntityChoice]) -> String {
    if choices.is_empty() {
        "none".to_string()
    } else {
        choices
            .iter()
            .map(|choice| format!("entity#{} {}", choice.id.index(), choice.label))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn first_move_destination(context: &ActorTurnContext) -> Option<PlaceId> {
    context
        .available_actions
        .iter()
        .find_map(|action| match action {
            AgentAvailableAction::MoveTo { destination } => Some(*destination),
            _ => None,
        })
}

fn first_nearby_entity(context: &ActorTurnContext) -> Option<EntityId> {
    context
        .available_actions
        .iter()
        .find_map(|action| match action {
            AgentAvailableAction::InspectEntity { entity_id } => Some(*entity_id),
            _ => None,
        })
}

fn preferred_speak_target(context: &ActorTurnContext) -> Option<ActorId> {
    let speakable_targets = context
        .available_actions
        .iter()
        .filter_map(|action| match action {
            AgentAvailableAction::SpeakTo { target } => Some(*target),
            _ => None,
        })
        .collect::<Vec<_>>();

    if let Some((speaker_id, _)) = latest_inbound_speech(context)
        && speakable_targets.contains(&speaker_id)
    {
        return Some(speaker_id);
    }

    context
        .local_state
        .nearby_actors
        .iter()
        .find(|actor| {
            actor.controller == crate::world::ControllerMode::Manual
                && speakable_targets.contains(&actor.id)
        })
        .map(|actor| actor.id)
        .or_else(|| speakable_targets.first().copied())
}

fn mock_reply_text(context: &ActorTurnContext, speaker_input: &str) -> String {
    let lower = speaker_input.to_lowercase();
    let mut lines = vec![format!(
        "{} the {} leans in, measuring your tone before answering.",
        context.actor.name(context.world_seed),
        context.actor.occupation.label()
    )];

    if lower.contains("job") || lower.contains("work") || lower.contains("favor") {
        lines.push(format!(
            "\"I might have something for you if you're reliable. Check around {} and see whether anything looks out of place.\"",
            crate::world::place_name_from_parts(
                context.world_seed,
                context.current_place.id,
                context.current_place.city_id,
                context.current_place.kind,
            )
        ));
    } else if lower.contains("where") || lower.contains("city") || lower.contains("travel") {
        lines.push(format!(
            "\"{} is a {} place built on {} and {}. I keep mostly to {}. From here you can push on toward {} if you've got a reason.\"",
            context.city.name(context.world_seed),
            context.city.biome.label(),
            context.city.economy.label(),
            context.city.culture.label(),
            context.actor.home_place_name(context.world_seed),
            if context.city.connected_cities.is_empty() {
                "nowhere worth naming".to_string()
            } else {
                context
                    .city
                    .connected_cities
                    .iter()
                    .map(|city| city.name(context.world_seed))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));
    } else {
        lines.push(format!(
            "\"You don't sound like most people passing through {}. That can be useful or it can get noticed. I spend most of my time around {}, so I hear things early.\"",
            context.city.name(context.world_seed),
            context.actor.home_place_name(context.world_seed)
        ));
    }

    lines.join(" ")
}

fn mock_initiation_text(context: &ActorTurnContext, target: ActorId) -> String {
    if target == context.actor.id {
        return "I'm talking to myself again.".to_string();
    }

    let target_name = target.name(context.world_seed);
    let place_name = place_name_from_parts(
        context.world_seed,
        context.current_place.id,
        context.current_place.city_id,
        context.current_place.kind,
    );
    let theme = match context.actor.goal.label() {
        "wander" => "room to room",
        "observe" => "what everyone is doing",
        "work" => "whether anything useful is happening",
        _ => "what matters around here",
    };

    format!(
        "{target_name}, before you drift off, tell me what you make of {place_name}. I've been thinking about {theme}."
    )
}

fn speaker_label(line: &DialogueLine) -> String {
    match line.speaker {
        DialogueSpeaker::Actor(_) => "Actor".to_string(),
        DialogueSpeaker::System => "System".to_string(),
    }
}

const TURN_PREAMBLE: &str = "You are choosing one autonomous NPC turn in a text game. You may initiate or continue conversation with speak_to, move to an adjacent room with move_to, inspect something nearby with inspect_entity, or stay idle with do_nothing. Recent speech matters, but does not force a reply. You must call exactly one real available tool. Never write plain text outside a tool call. Never narrate. Never explain. Call one tool and stop.";
const TURN_MAX_TOKENS: u64 = 224;
const TURN_SELECTION_TIMEOUT: Duration = Duration::from_secs(60);
const TURN_POLL_INTERVAL: Duration = Duration::from_millis(250);
const TURN_POST_COMMIT_GRACE: Duration = Duration::from_secs(2);
const MEMORY_MAX_TOKENS: u64 = 96;
const MEMORY_SUMMARY_TIMEOUT: Duration = Duration::from_secs(4);
const MEMORY_PREAMBLE: &str =
    "Summarize conversations for a text game. Keep only durable memory of what was discussed.";

fn fallback_summary(transcript: &[DialogueLine]) -> String {
    let summary = transcript
        .iter()
        .rev()
        .take(6)
        .rev()
        .map(|line| format!("{}: {}", speaker_label(line), line.text))
        .collect::<Vec<_>>()
        .join(" | ");

    if summary.is_empty() {
        "No memorable conversation yet.".to_string()
    } else {
        summary
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use petgraph::stable_graph::NodeIndex;
    use rig::tool::Tool;
    use serde_json::json;

    use crate::ai::context::build_actor_turn_context;
    use crate::domain::commands::AgentAvailableAction;
    use crate::domain::events::{DialogueLine, DialogueSpeaker};
    use crate::domain::seed::WorldSeed;
    use crate::domain::time::{GameTime, TimeDelta};
    use crate::world::{ControllerMode, World};

    use super::{
        ActionKind, ActorTargetChoice, DoNothingTool, EntityChoice, InspectEntityArgs,
        InspectEntityTool, LlmBackend, MockBackend, MoveToArgs, MoveToTool, PlaceChoice,
        SpeakToArgs, SpeakToTool,
    };

    fn ai_actor_id(world: &World) -> crate::world::ActorId {
        world
            .actor_ids()
            .into_iter()
            .find(|candidate| world.actor(*candidate).controller == ControllerMode::AiAgent)
            .expect("expected an ai actor")
    }

    #[tokio::test]
    async fn mock_backend_chooses_speak_when_addressed() {
        let mut world = World::generate(WorldSeed::new(2), 16);
        let actor_id = world.manual_actor_id().unwrap();
        let place_id = world.actor_place_id(actor_id).unwrap();
        let counterpart_id = world
            .place_actors(place_id)
            .into_iter()
            .find(|candidate| *candidate != actor_id)
            .unwrap_or_else(|| {
                let counterpart_id = world
                    .actor_ids()
                    .into_iter()
                    .find(|candidate| *candidate != actor_id)
                    .unwrap();
                world.move_actor(counterpart_id, place_id);
                counterpart_id
            });
        world.record_speech_process(
            actor_id,
            counterpart_id,
            place_id,
            GameTime::from_seconds(0),
            TimeDelta::from_seconds(10),
            vec![DialogueLine {
                timestamp: GameTime::from_seconds(0),
                speaker: DialogueSpeaker::Actor(actor_id),
                text: "Hello".to_string(),
            }],
        );

        let context = build_actor_turn_context(
            &world,
            GameTime::from_seconds(10),
            counterpart_id,
            vec![
                AgentAvailableAction::SpeakTo { target: actor_id },
                AgentAvailableAction::DoNothing,
            ],
        )
        .unwrap();

        let action = MockBackend.choose_action(&context).await.unwrap();
        assert!(matches!(
            action.action,
            ActionKind::Speak { target, .. } if target == actor_id
        ));
        assert_eq!(action.trace.actor_id, counterpart_id);
    }

    #[tokio::test]
    async fn mock_backend_can_proactively_speak_without_inbound_speech() {
        let world = World::generate(WorldSeed::new(4), 16);
        let actor_id = ai_actor_id(&world);
        let target_id = world.manual_actor_id().unwrap();
        let context = build_actor_turn_context(
            &world,
            GameTime::from_seconds(0),
            actor_id,
            vec![
                AgentAvailableAction::SpeakTo { target: target_id },
                AgentAvailableAction::DoNothing,
            ],
        )
        .unwrap();

        let action = MockBackend.choose_action(&context).await.unwrap();
        assert!(matches!(
            action.action,
            ActionKind::Speak { target, .. } if target == target_id
        ));
        assert!(action.trace.toolset.iter().any(|tool| tool == "speak_to"));
    }

    #[tokio::test]
    async fn mock_backend_can_choose_move() {
        let world = World::generate(WorldSeed::new(5), 16);
        let actor_id = ai_actor_id(&world);
        let destination = world.place_routes(world.actor_place_id(actor_id).unwrap())[0].0;
        let context = build_actor_turn_context(
            &world,
            GameTime::from_seconds(0),
            actor_id,
            vec![
                AgentAvailableAction::MoveTo { destination },
                AgentAvailableAction::DoNothing,
            ],
        )
        .unwrap();

        let action = MockBackend.choose_action(&context).await.unwrap();
        assert!(matches!(
            action.action,
            ActionKind::MoveTo { destination: chosen } if chosen == destination
        ));
    }

    #[tokio::test]
    async fn mock_backend_can_choose_inspect() {
        let mut world = World::generate(WorldSeed::new(6), 16);
        let actor_id = ai_actor_id(&world);
        let place_id = world.actor_place_id(actor_id).unwrap();
        let entity_id = world
            .place_entities(place_id)
            .into_iter()
            .next()
            .unwrap_or_else(|| {
                let other_place = world
                    .city_places(world.place_city_id(place_id).unwrap())
                    .into_iter()
                    .find(|candidate| !world.place_entities(*candidate).is_empty())
                    .expect("expected a place with an entity");
                world.move_actor(actor_id, other_place);
                world.place_entities(other_place)[0]
            });

        let context = build_actor_turn_context(
            &world,
            GameTime::from_seconds(0),
            actor_id,
            vec![
                AgentAvailableAction::InspectEntity { entity_id },
                AgentAvailableAction::DoNothing,
            ],
        )
        .unwrap();

        let action = MockBackend.choose_action(&context).await.unwrap();
        assert!(matches!(
            action.action,
            ActionKind::InspectEntity { entity_id: chosen } if chosen == entity_id
        ));
    }

    #[tokio::test]
    async fn mock_backend_can_choose_do_nothing() {
        let world = World::generate(WorldSeed::new(3), 16);
        let actor_id = ai_actor_id(&world);
        let context = build_actor_turn_context(
            &world,
            GameTime::from_seconds(0),
            actor_id,
            vec![AgentAvailableAction::DoNothing],
        )
        .unwrap();

        let action = MockBackend.choose_action(&context).await.unwrap();
        assert_eq!(action.action, ActionKind::DoNothing);
    }

    #[tokio::test]
    async fn speak_to_tool_commits_speak_action() {
        let selected_action = Arc::new(Mutex::new(None));
        let tool_calls = Arc::new(Mutex::new(Vec::new()));
        let actor_id = crate::world::ActorId(NodeIndex::new(1));
        let target_id = crate::world::ActorId(NodeIndex::new(2));
        let tool = SpeakToTool::new(
            actor_id,
            vec![ActorTargetChoice {
                id: target_id,
                label: "target".to_string(),
            }],
            Arc::clone(&selected_action),
            Arc::clone(&tool_calls),
        );

        let output = tool
            .call(SpeakToArgs {
                target_actor_id: target_id.index(),
                text: "hello there".to_string(),
            })
            .await
            .unwrap();

        assert!(matches!(
            output.action,
            ActionKind::Speak { target, .. } if target == target_id
        ));
        assert!(matches!(
            selected_action.lock().unwrap().clone(),
            Some(ActionKind::Speak { target, .. }) if target == target_id
        ));
    }

    #[tokio::test]
    async fn move_to_tool_commits_move_action() {
        let selected_action = Arc::new(Mutex::new(None));
        let tool_calls = Arc::new(Mutex::new(Vec::new()));
        let actor_id = crate::world::ActorId(NodeIndex::new(1));
        let destination = crate::world::PlaceId(NodeIndex::new(8));
        let tool = MoveToTool::new(
            actor_id,
            vec![PlaceChoice {
                id: destination,
                label: "hallway".to_string(),
            }],
            Arc::clone(&selected_action),
            Arc::clone(&tool_calls),
        );

        let output = tool
            .call(MoveToArgs {
                destination_place_id: destination.index(),
            })
            .await
            .unwrap();

        assert!(matches!(
            output.action,
            ActionKind::MoveTo { destination: chosen } if chosen == destination
        ));
    }

    #[tokio::test]
    async fn inspect_entity_tool_commits_inspect_action() {
        let selected_action = Arc::new(Mutex::new(None));
        let tool_calls = Arc::new(Mutex::new(Vec::new()));
        let actor_id = crate::world::ActorId(NodeIndex::new(1));
        let entity_id = crate::world::EntityId(NodeIndex::new(4));
        let tool = InspectEntityTool::new(
            actor_id,
            vec![EntityChoice {
                id: entity_id,
                label: "locker".to_string(),
            }],
            Arc::clone(&selected_action),
            Arc::clone(&tool_calls),
        );

        let output = tool
            .call(InspectEntityArgs {
                entity_id: entity_id.index(),
            })
            .await
            .unwrap();

        assert!(matches!(
            output.action,
            ActionKind::InspectEntity { entity_id: chosen } if chosen == entity_id
        ));
    }

    #[tokio::test]
    async fn duplicate_tool_commits_fail_cleanly() {
        let selected_action = Arc::new(Mutex::new(None));
        let tool_calls = Arc::new(Mutex::new(Vec::new()));
        let actor_id = crate::world::ActorId(NodeIndex::new(1));
        let target_id = crate::world::ActorId(NodeIndex::new(2));
        let speak_tool = SpeakToTool::new(
            actor_id,
            vec![ActorTargetChoice {
                id: target_id,
                label: "target".to_string(),
            }],
            Arc::clone(&selected_action),
            Arc::clone(&tool_calls),
        );
        let idle_tool = DoNothingTool::new(
            actor_id,
            Arc::clone(&selected_action),
            Arc::clone(&tool_calls),
        );

        speak_tool
            .call(SpeakToArgs {
                target_actor_id: target_id.index(),
                text: "hello".to_string(),
            })
            .await
            .unwrap();
        let err = idle_tool.call(json!({})).await.unwrap_err();

        assert!(err.to_string().contains("already selected"));
        assert_eq!(tool_calls.lock().unwrap().len(), 2);
        assert!(tool_calls.lock().unwrap()[1].error.is_some());
    }

    #[test]
    fn fallback_summary_keeps_recent_lines() {
        let summary = super::fallback_summary(&[
            DialogueLine {
                timestamp: GameTime::from_seconds(0),
                speaker: DialogueSpeaker::System,
                text: "alpha".to_string(),
            },
            DialogueLine {
                timestamp: GameTime::from_seconds(1),
                speaker: DialogueSpeaker::System,
                text: "beta".to_string(),
            },
        ]);

        assert!(summary.contains("alpha"));
        assert!(summary.contains("beta"));
    }
}
