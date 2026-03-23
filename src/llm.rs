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
use crate::world::ActorId;
const CONVERSATION_TOOL_NAMES: &[&str] = &["reply_to_speaker", "do_nothing"];

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
        let Some((speaker_id, speaker_input)) = latest_inbound_speech(context) else {
            trace.toolset = vec!["do_nothing".to_string()];
            trace.model_output = Some("mock policy: no recent inbound speech, staying idle".to_string());
            trace.selected_action = Some(ActionKind::DoNothing);
            return Ok(ActionSelection {
                action: ActionKind::DoNothing,
                trace,
            });
        };

        if !context
            .available_actions
            .contains(&AgentAvailableAction::SpeakTo { target: speaker_id })
        {
            trace.toolset = vec!["do_nothing".to_string()];
            trace.model_output = Some(
                "mock policy: latest speaker is not currently speakable, staying idle".to_string(),
            );
            trace.selected_action = Some(ActionKind::DoNothing);
            return Ok(ActionSelection {
                action: ActionKind::DoNothing,
                trace,
            });
        }

        let action = ActionKind::Speak {
            target: speaker_id,
            text: mock_reply_text(context, &speaker_input),
        };
        trace.toolset = CONVERSATION_TOOL_NAMES
            .iter()
            .map(|name| (*name).to_string())
            .collect();
        trace.model_output = Some("mock policy: replying to latest inbound speech".to_string());
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

    async fn conversation_choose_action(
        &self,
        context: &ActorTurnContext,
        speaker_id: ActorId,
        speaker_input: &str,
    ) -> Result<ActionSelection> {
        let prompt = build_conversation_prompt(context, speaker_id, speaker_input);
        debug!(
            actor_id = context.actor.id.index(),
            speaker_id = speaker_id.index(),
            backend = %self.label(),
            prompt_length = prompt.len(),
            "starting rig conversation action selection"
        );
        trace!(
            actor_id = context.actor.id.index(),
            speaker_id = speaker_id.index(),
            backend = %self.label(),
            prompt = %prompt,
            "conversation prompt"
        );
        let selected_action = Arc::new(Mutex::new(None));
        let tool_calls = Arc::new(Mutex::new(Vec::new()));
        let tools: Vec<Box<dyn ToolDyn>> = vec![
            Box::new(ReplyToSpeakerTool::new(
                speaker_id,
                Arc::clone(&selected_action),
                Arc::clone(&tool_calls),
            )),
            Box::new(DoNothingTool::new(
                Arc::clone(&selected_action),
                Arc::clone(&tool_calls),
            )),
        ];
        debug!(
            actor_id = context.actor.id.index(),
            speaker_id = speaker_id.index(),
            backend = %self.label(),
            toolset = ?CONVERSATION_TOOL_NAMES,
            "conversation tools prepared"
        );
        let request_started = Instant::now();
        let request_future = async {
            match &self.provider {
                RigProvider::Ollama { client, model } => {
                    debug!(
                        actor_id = context.actor.id.index(),
                        speaker_id = speaker_id.index(),
                        backend = %self.label(),
                        model,
                        timeout_seconds = CONVERSATION_SELECTION_TIMEOUT.as_secs(),
                        "dispatching ollama conversation agent request"
                    );
                    let agent = client
                        .agent(model.clone())
                        .preamble(CONVERSATION_PREAMBLE)
                        .temperature(0.0)
                        .max_tokens(CONVERSATION_MAX_TOKENS)
                        .additional_params(json!({ "think": false }))
                        .tools(tools)
                        .build();
                    agent.prompt(prompt.clone()).max_turns(1).await
                }
                RigProvider::OpenAiCompatible { client, model } => {
                    debug!(
                        actor_id = context.actor.id.index(),
                        speaker_id = speaker_id.index(),
                        backend = %self.label(),
                        model,
                        timeout_seconds = CONVERSATION_SELECTION_TIMEOUT.as_secs(),
                        "dispatching openai-compatible conversation agent request"
                    );
                    let agent = client
                        .agent(model.clone())
                        .preamble(CONVERSATION_PREAMBLE)
                        .temperature(0.0)
                        .max_tokens(CONVERSATION_MAX_TOKENS)
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
            if request_started.elapsed() >= CONVERSATION_SELECTION_TIMEOUT {
                timed_out = true;
                break;
            }

            tokio::select! {
                result = &mut request_future => {
                    completion = Some(result);
                    break;
                }
                _ = tokio::time::sleep(CONVERSATION_POLL_INTERVAL) => {
                    let has_selected = selected_action
                        .lock()
                        .map_err(|_| anyhow!("conversation action selection lock poisoned"))?
                        .is_some();
                    if has_selected {
                        let selected_since = selected_seen_at.get_or_insert_with(Instant::now);
                        debug!(
                            actor_id = context.actor.id.index(),
                            speaker_id = speaker_id.index(),
                            backend = %self.label(),
                            since_commit_ms = selected_since.elapsed().as_millis(),
                            elapsed_ms = request_started.elapsed().as_millis(),
                            "conversation action committed while provider request is still running"
                        );
                        if selected_since.elapsed() >= CONVERSATION_POST_COMMIT_GRACE {
                            cancelled_after_commit = true;
                            info!(
                                actor_id = context.actor.id.index(),
                                speaker_id = speaker_id.index(),
                                backend = %self.label(),
                                since_commit_ms = selected_since.elapsed().as_millis(),
                                elapsed_ms = request_started.elapsed().as_millis(),
                                "ending conversation request after committed-action grace period"
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
            .map_err(|_| anyhow!("conversation action selection lock poisoned"))?
            .clone();
        let action = selected.clone().unwrap_or(ActionKind::DoNothing);
        let mut trace = AgentDebugTrace::from_context(context, &self.label());
        trace.prompt = prompt.clone();
        trace.toolset = CONVERSATION_TOOL_NAMES
            .iter()
            .map(|name| (*name).to_string())
            .collect();
        trace.tool_calls = tool_calls
            .lock()
            .map_err(|_| anyhow!("tool call trace lock poisoned"))?
            .clone();
        debug!(
            actor_id = context.actor.id.index(),
            speaker_id = speaker_id.index(),
            backend = %self.label(),
            elapsed_ms = request_elapsed.as_millis(),
            selected_action_present = selected.is_some(),
            tool_call_count = trace.tool_calls.len(),
            "conversation agent request completed"
        );

        match completion {
            Some(Ok(model_output)) => {
                debug!(
                    actor_id = context.actor.id.index(),
                    speaker_id = speaker_id.index(),
                    backend = %self.label(),
                    elapsed_ms = request_elapsed.as_millis(),
                    model_output_len = model_output.len(),
                    "conversation agent returned a completion payload"
                );
                trace!(
                    actor_id = context.actor.id.index(),
                    speaker_id = speaker_id.index(),
                    backend = %self.label(),
                    model_output = %model_output,
                    "conversation agent raw completion"
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
                        "conversation agent returned an error after committing an action; accepting the committed action"
                    );
                } else {
                    warn!(
                        actor_id = context.actor.id.index(),
                        backend = %self.label(),
                        error = %error,
                        elapsed_ms = request_elapsed.as_millis(),
                        "conversation agent failed without a committed action"
                    );
                }
            }
            None if timed_out => {
                let timeout_message = format!(
                    "conversation selection timed out after {}s",
                    CONVERSATION_SELECTION_TIMEOUT.as_secs()
                );
                trace.error = Some(timeout_message);
                if selected.is_some() {
                    warn!(
                        actor_id = context.actor.id.index(),
                        backend = %self.label(),
                        selected_action = ?action,
                        timeout_seconds = CONVERSATION_SELECTION_TIMEOUT.as_secs(),
                        elapsed_ms = request_elapsed.as_millis(),
                        "conversation agent timed out after committing an action; accepting the committed action"
                    );
                } else {
                    warn!(
                        actor_id = context.actor.id.index(),
                        backend = %self.label(),
                        timeout_seconds = CONVERSATION_SELECTION_TIMEOUT.as_secs(),
                        elapsed_ms = request_elapsed.as_millis(),
                        "conversation agent timed out without a committed action"
                    );
                }
            }
            None if cancelled_after_commit => {
                info!(
                    actor_id = context.actor.id.index(),
                    speaker_id = speaker_id.index(),
                    backend = %self.label(),
                    elapsed_ms = request_elapsed.as_millis(),
                    tool_call_count = trace.tool_calls.len(),
                    "conversation request ended after committed-action grace period"
                );
            }
            None => unreachable!("conversation request should either complete, time out, or cancel after commit"),
        }

        if selected.is_none() && matches!(action, ActionKind::DoNothing) {
            let prior_error = trace.error.take();
            trace.error = Some(match prior_error {
                Some(error) => format!("{error}; no tool committed an action"),
                None => "no tool committed an action".to_string(),
            });
            warn!(
                actor_id = context.actor.id.index(),
                speaker_id = speaker_id.index(),
                backend = %self.label(),
                elapsed_ms = request_elapsed.as_millis(),
                model_output_present = trace.model_output.is_some(),
                tool_call_count = trace.tool_calls.len(),
                "conversation agent completed without a committed tool action"
            );
        }

        trace.selected_action = Some(action.clone());
        info!(
            actor_id = context.actor.id.index(),
            backend = %self.label(),
            selected_action = ?action,
            tool_call_count = trace.tool_calls.len(),
            elapsed_ms = request_elapsed.as_millis(),
            "rig conversation action selection finished"
        );
        trace!(
            actor_id = context.actor.id.index(),
            backend = %self.label(),
            tool_calls = ?trace.tool_calls,
            "rig conversation tool trace"
        );

        Ok(ActionSelection { action, trace })
    }

    async fn agent_choose_action(&self, context: &ActorTurnContext) -> Result<ActionSelection> {
        if let Some((speaker_id, speaker_input)) = latest_inbound_speech(context)
            && context
                .available_actions
                .contains(&AgentAvailableAction::SpeakTo { target: speaker_id })
        {
            debug!(
                actor_id = context.actor.id.index(),
                speaker_id = speaker_id.index(),
                backend = %self.label(),
                available_actions = ?context.available_actions,
                "recent inbound speech detected; entering conversation tool path"
            );
            return self
                .conversation_choose_action(context, speaker_id, &speaker_input)
                .await;
        }

        let mut trace = AgentDebugTrace::from_context(context, &self.label());
        trace.toolset = vec!["do_nothing".to_string()];
        trace.model_output = Some("no recent inbound speech; staying idle".to_string());
        trace.selected_action = Some(ActionKind::DoNothing);
        debug!(
            actor_id = context.actor.id.index(),
            backend = %self.label(),
            "no recent inbound speech; returning do_nothing"
        );
        Ok(ActionSelection {
            action: ActionKind::DoNothing,
            trace,
        })
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
struct ReplyAcknowledgement {
    accepted: bool,
    action: ActionKind,
}

#[derive(Debug, Clone, Serialize)]
struct IdleAcknowledgement {
    accepted: bool,
    action: ActionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReplyToSpeakerArgs {
    text: String,
}

type ToolTraceSink = Arc<Mutex<Vec<AgentToolTrace>>>;

struct ReplyToSpeakerTool {
    speaker_id: ActorId,
    selected_action: Arc<Mutex<Option<ActionKind>>>,
    tool_calls: ToolTraceSink,
}

impl ReplyToSpeakerTool {
    fn new(
        speaker_id: ActorId,
        selected_action: Arc<Mutex<Option<ActionKind>>>,
        tool_calls: ToolTraceSink,
    ) -> Self {
        Self {
            speaker_id,
            selected_action,
            tool_calls,
        }
    }
}

impl Tool for ReplyToSpeakerTool {
    const NAME: &'static str = "reply_to_speaker";
    type Error = IoError;
    type Args = ReplyToSpeakerArgs;
    type Output = ReplyAcknowledgement;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Reply directly to the actor who just spoke to you. Provide only the words you want to say.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The exact short reply to say back to the speaker."
                    }
                },
                "required": ["text"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        debug!(
            tool_name = Self::NAME,
            target_actor_id = self.speaker_id.index(),
            text_len = args.text.len(),
            "reply_to_speaker invoked"
        );
        let text = args.text.trim();
        if text.is_empty() {
            let error = "reply_to_speaker requires non-empty text".to_string();
            push_tool_trace(&self.tool_calls, Self::NAME, &args, None, Some(error.clone()));
            return Err(IoError::other(error));
        }

        let action = ActionKind::Speak {
            target: self.speaker_id,
            text: text.to_string(),
        };
        let mut guard = self
            .selected_action
            .lock()
            .map_err(|_| IoError::other("selected action lock poisoned"))?;
        if guard.is_some() {
            let error = "an action was already selected".to_string();
            push_tool_trace(&self.tool_calls, Self::NAME, &args, None, Some(error.clone()));
            return Err(IoError::other(error));
        }
        *guard = Some(action.clone());
        debug!(
            tool_name = Self::NAME,
            target_actor_id = self.speaker_id.index(),
            "reply_to_speaker committed action"
        );

        let output = ReplyAcknowledgement {
            accepted: true,
            action,
        };
        push_tool_trace(
            &self.tool_calls,
            Self::NAME,
            &args,
            Some(serialize_pretty(&output)),
            None,
        );
        Ok(output)
    }
}

struct DoNothingTool {
    selected_action: Arc<Mutex<Option<ActionKind>>>,
    tool_calls: ToolTraceSink,
}

impl DoNothingTool {
    fn new(
        selected_action: Arc<Mutex<Option<ActionKind>>>,
        tool_calls: ToolTraceSink,
    ) -> Self {
        Self {
            selected_action,
            tool_calls,
        }
    }
}

impl Tool for DoNothingTool {
    const NAME: &'static str = "do_nothing";
    type Error = IoError;
    type Args = serde_json::Value;
    type Output = IdleAcknowledgement;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Decline to answer or act right now.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        debug!(tool_name = Self::NAME, "do_nothing invoked");
        let mut guard = self
            .selected_action
            .lock()
            .map_err(|_| IoError::other("selected action lock poisoned"))?;
        if guard.is_some() {
            let error = "an action was already selected".to_string();
            push_tool_trace(&self.tool_calls, Self::NAME, &args, None, Some(error.clone()));
            return Err(IoError::other(error));
        }

        let action = ActionKind::DoNothing;
        *guard = Some(action.clone());
        debug!(tool_name = Self::NAME, "do_nothing committed action");

        let output = IdleAcknowledgement {
            accepted: true,
            action,
        };
        push_tool_trace(
            &self.tool_calls,
            Self::NAME,
            &args,
            Some(serialize_pretty(&output)),
            None,
        );
        Ok(output)
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

fn build_conversation_prompt(
    context: &ActorTurnContext,
    speaker_id: ActorId,
    speaker_input: &str,
) -> String {
    format!(
        "You are {} the {} in {}.\nTraits: {}\nGoal: {}\nMemory: {}\n{} just said: {}\nDecide whether to answer right now.\nYou MUST call exactly one tool.\nIf you answer, call reply_to_speaker with only the exact words you say, 1-3 short sentences, no narration, no speaker labels, no stage directions.\nIf you do not want to answer, call do_nothing.\nDo not write plain text outside a tool call.\nDo not call both tools.\nCall one tool and stop.",
        context.actor.name(context.world_seed),
        context.actor.occupation.label(),
        crate::world::place_name_from_parts(
            context.world_seed,
            context.current_place.id,
            context.current_place.city_id,
            context.current_place.kind,
        ),
        context
            .actor
            .traits
            .iter()
            .map(|trait_tag| trait_tag.label())
            .collect::<Vec<_>>()
            .join(", "),
        context.actor.goal.label(),
        render_memory_for_prompt(&context.memory),
        speaker_id.name(context.world_seed),
        speaker_input.trim(),
    )
}

fn render_memory_for_prompt(memory: &ConversationMemory) -> String {
    let summary = memory.summary.trim();
    if summary.is_empty() {
        "none".to_string()
    } else {
        summary.to_string()
    }
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

fn speaker_label(line: &DialogueLine) -> String {
    match line.speaker {
        DialogueSpeaker::Actor(_) => "Actor".to_string(),
        DialogueSpeaker::System => "System".to_string(),
    }
}

const CONVERSATION_PREAMBLE: &str = "You are handling one immediate conversation turn for an NPC in a text game. You must choose exactly one real tool call. Use reply_to_speaker to answer in character, or do_nothing to stay silent. Never answer in plain text. Do not narrate. Do not explain. Call one tool and stop.";
const CONVERSATION_MAX_TOKENS: u64 = 192;
const CONVERSATION_SELECTION_TIMEOUT: Duration = Duration::from_secs(60);
const CONVERSATION_POLL_INTERVAL: Duration = Duration::from_millis(250);
const CONVERSATION_POST_COMMIT_GRACE: Duration = Duration::from_secs(2);
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
    use crate::ai::context::build_actor_turn_context;
    use crate::domain::commands::AgentAvailableAction;
    use crate::domain::events::{DialogueLine, DialogueSpeaker};
    use crate::domain::seed::WorldSeed;
    use crate::domain::time::{GameTime, TimeDelta};
    use crate::world::World;

    use super::{ActionKind, LlmBackend, MockBackend};

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
    async fn mock_backend_can_choose_do_nothing() {
        let world = World::generate(WorldSeed::new(3), 16);
        let actor_id = world
            .actor_ids()
            .into_iter()
            .find(|candidate| world.actor(*candidate).controller == crate::world::ControllerMode::AiAgent)
            .unwrap();
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
