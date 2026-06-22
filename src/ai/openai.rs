// Copyright 2026 The Sashiko Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::ai::token_budget::TokenBudget;
use crate::ai::{
    AiErrorClass, AiProvider, AiRequest, AiResponse, AiResponseFormat, AiRole, AiUsage,
    ClassifyAiError, ProviderCapabilities, ToolCall, classify_status_code,
};
use crate::utils::redact_secret;
use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAiRequest {
    pub model: String,
    pub messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAiMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAiToolCallFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAiToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAiTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAiFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAiFunction {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiResponse {
    pub choices: Vec<OpenAiChoice>,
    pub usage: OpenAiUsage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiChoice {
    pub index: u32,
    pub message: OpenAiMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAiUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChunk {
    #[serde(default)]
    choices: Vec<OpenAiStreamChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    #[serde(rename = "index")]
    _index: u32,
    delta: OpenAiDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct OpenAiDelta {
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCallDelta {
    index: u32,
    id: Option<String>,
    #[serde(rename = "type")]
    tool_type: Option<String>,
    function: Option<OpenAiToolCallFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCallFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamError {
    error: OpenAiStreamErrorBody,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamErrorBody {
    message: Option<String>,
    #[serde(rename = "type")]
    error_type: Option<String>,
    code: Option<Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenAiCompatError {
    #[error("Rate limit exceeded, retry after {0:?}")]
    RateLimitExceeded(Duration),
    #[error("Transient error: {1}, retry after {0:?}")]
    TransientError(Duration, String),
    #[error("Authentication error: {0}")]
    AuthenticationError(String),
    #[error("API error {0}: {1}")]
    ApiError(reqwest::StatusCode, String),
}

impl ClassifyAiError for OpenAiCompatError {
    fn ai_error_class(&self) -> AiErrorClass {
        match self {
            OpenAiCompatError::RateLimitExceeded(retry_after) => AiErrorClass::RateLimit {
                retry_after: *retry_after,
            },
            OpenAiCompatError::TransientError(retry_after, _) => AiErrorClass::Transient {
                retry_after: *retry_after,
            },
            OpenAiCompatError::AuthenticationError(_) => AiErrorClass::Fatal,
            OpenAiCompatError::ApiError(status, _) => {
                classify_status_code(*status).unwrap_or(AiErrorClass::Fatal)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiProviderType {
    /// Official OpenAI API — uses `max_completion_tokens`.
    OpenAi,
    /// Third-party OpenAI-compatible APIs — uses `max_tokens`.
    OpenAiCompatible,
}

pub struct OpenAiCompatClient {
    model: String,
    base_url: String,
    context_window_size: usize,
    max_tokens: u32,
    provider_type: OpenAiProviderType,
    client: Client,
    streaming: bool,
    stream_idle_timeout_secs: u64,
}

impl OpenAiCompatClient {
    pub fn new(
        base_url: String,
        provider_type: OpenAiProviderType,
        model: String,
        context_window_size: usize,
        max_tokens: u32,
        api_timeout_secs: u64,
        streaming: bool,
        stream_idle_timeout_secs: u64,
    ) -> Self {
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("LLM_API_KEY"))
            .unwrap_or_default();

        let mut headers = reqwest::header::HeaderMap::new();
        if !api_key.is_empty()
            && let Ok(value) =
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", api_key))
        {
            headers.insert("Authorization", value);
        }

        let mut builder = reqwest::Client::builder().default_headers(headers);
        if streaming {
            let connect_timeout_secs = api_timeout_secs.clamp(1, 60);
            builder = builder.connect_timeout(Duration::from_secs(connect_timeout_secs));
        } else {
            builder = builder.timeout(Duration::from_secs(api_timeout_secs));
        }
        let client = builder.build().unwrap_or_else(|_| reqwest::Client::new());

        let base_url = Self::normalize_base_url(&base_url);

        Self {
            model,
            base_url,
            context_window_size,
            max_tokens,
            provider_type,
            client,
            streaming,
            stream_idle_timeout_secs,
        }
    }

    /// Normalize a base URL so it always ends with `/chat/completions`.
    ///
    /// LM Studio and other OpenAI-compatible servers document the base URL as
    /// `http://localhost:1234/v1`, expecting the client to append the endpoint
    /// path.  Our `post_request` POSTs directly to `self.base_url`, so we
    /// ensure the full path is present.
    fn normalize_base_url(url: &str) -> String {
        let url = url.trim_end_matches('/');
        if url.ends_with("/chat/completions") {
            url.to_string()
        } else {
            format!("{}/chat/completions", url)
        }
    }

    pub fn default_base_url_for_model(model: &str) -> String {
        if model.starts_with("glm-") {
            "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string()
        } else if model.starts_with("moonshot-") {
            "https://api.moonshot.cn/v1/chat/completions".to_string()
        } else if model.starts_with("abab7-") || model.starts_with("MiniMax-") {
            "https://api.minimax.chat/v1/text/chatcompletion_v2".to_string()
        } else {
            "https://api.openai.com/v1/chat/completions".to_string()
        }
    }

    pub fn default_context_window_for_model(model: &str) -> usize {
        if model.starts_with("glm-") || model.starts_with("moonshot-") {
            128_000
        } else if model.starts_with("abab7-") || model.starts_with("MiniMax-") {
            245_760
        } else if model.starts_with("gpt-4o") || model.starts_with("gpt-4-turbo") {
            128_000
        } else if model.starts_with("gpt-3.5") {
            16_385
        } else {
            128_000
        }
    }

    async fn error_from_response(res: reqwest::Response) -> OpenAiCompatError {
        let re = Regex::new(r"Please retry in ([0-9.]+)s").unwrap();
        let status = res.status();
        let status_code = status.as_u16();

        let retry_after_duration = res
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs);

        let error_text = redact_secret(&res.text().await.unwrap_or_default());

        match status_code {
            429 => {
                let mut retry_seconds = retry_after_duration
                    .unwrap_or(Duration::from_secs(60))
                    .as_secs_f64();
                if let Some(caps) = re.captures(&error_text) {
                    retry_seconds = caps[1].parse::<f64>().unwrap_or(retry_seconds);
                }
                tracing::warn!("OpenAI 429 Rate Limit. Retry in {}s", retry_seconds);
                OpenAiCompatError::RateLimitExceeded(Duration::from_secs_f64(retry_seconds))
            }
            401 | 403 => OpenAiCompatError::AuthenticationError(error_text),
            500..=599 => {
                tracing::warn!("OpenAI Server Error {}: {}", status, error_text);
                OpenAiCompatError::TransientError(
                    retry_after_duration.unwrap_or(Duration::from_secs(0)),
                    error_text,
                )
            }
            _ => OpenAiCompatError::ApiError(status, error_text),
        }
    }

    async fn post_request(&self, body: &Value) -> Result<OpenAiResponse, OpenAiCompatError> {
        let res = match self.client.post(&self.base_url).json(body).send().await {
            Ok(res) => res,
            Err(e) => {
                let err_str = redact_secret(&e.to_string());
                tracing::error!("OpenAI request failed (transport): {}", err_str);
                return Err(OpenAiCompatError::TransientError(
                    Duration::from_secs(30),
                    err_str,
                ));
            }
        };

        if res.status().is_success() {
            let status = res.status();
            let body_text = res.text().await.map_err(|e| {
                let err_str = redact_secret(&e.to_string());
                tracing::error!("Failed to read OpenAI response body: {}", err_str);
                OpenAiCompatError::TransientError(Duration::from_secs(30), err_str)
            })?;
            match serde_json::from_str::<OpenAiResponse>(&body_text) {
                Ok(response) => {
                    tracing::info!(
                        "OpenAI response received. Tokens: in={}, out={}",
                        response.usage.prompt_tokens,
                        response.usage.completion_tokens
                    );
                    return Ok(response);
                }
                Err(e) => {
                    tracing::error!("Failed to decode OpenAI response: {}", e);
                    return Err(OpenAiCompatError::ApiError(
                        status,
                        format!("Parse error: {}", e),
                    ));
                }
            }
        }

        Err(Self::error_from_response(res).await)
    }

    async fn post_stream_request(
        &self,
        body: &Value,
        prompt_tokens: u32,
    ) -> Result<OpenAiResponse, OpenAiCompatError> {
        let res = match self.client.post(&self.base_url).json(body).send().await {
            Ok(res) => res,
            Err(e) => {
                let err_str = redact_secret(&e.to_string());
                tracing::error!("OpenAI streaming request failed (transport): {}", err_str);
                return Err(OpenAiCompatError::TransientError(
                    Duration::from_secs(30),
                    err_str,
                ));
            }
        };

        if !res.status().is_success() {
            return Err(Self::error_from_response(res).await);
        }

        let mut stream = res.bytes_stream();
        let mut parser = OpenAiSseStreamParser::default();
        loop {
            let next = if self.stream_idle_timeout_secs == 0 {
                stream.next().await
            } else {
                match tokio::time::timeout(
                    Duration::from_secs(self.stream_idle_timeout_secs),
                    stream.next(),
                )
                .await
                {
                    Ok(next) => next,
                    Err(_) => {
                        return Err(OpenAiCompatError::TransientError(
                            Duration::from_secs(30),
                            format!(
                                "OpenAI stream idle timeout after {} seconds",
                                self.stream_idle_timeout_secs
                            ),
                        ));
                    }
                }
            };

            match next {
                Some(Ok(chunk)) => {
                    if let Some(response) = parser.push_bytes(&chunk, prompt_tokens)? {
                        tracing::info!(
                            "OpenAI streaming response received. Tokens: in={}, out={}",
                            response.usage.prompt_tokens,
                            response.usage.completion_tokens
                        );
                        return Ok(response);
                    }
                }
                Some(Err(e)) => {
                    let err_str = redact_secret(&e.to_string());
                    tracing::error!("Failed to read OpenAI streaming chunk: {}", err_str);
                    return Err(OpenAiCompatError::TransientError(
                        Duration::from_secs(30),
                        err_str,
                    ));
                }
                None => break,
            }
        }

        let response = parser.finish(prompt_tokens)?;
        tracing::info!(
            "OpenAI streaming response received. Tokens: in={}, out={}",
            response.usage.prompt_tokens,
            response.usage.completion_tokens
        );
        Ok(response)
    }
}

#[derive(Debug, Default)]
struct OpenAiStreamAccumulator {
    role: Option<String>,
    content: String,
    tool_calls: BTreeMap<u32, OpenAiToolCallAccumulator>,
    finish_reason: Option<String>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Default)]
struct OpenAiToolCallAccumulator {
    id: Option<String>,
    tool_type: Option<String>,
    name: Option<String>,
    arguments: String,
}

#[derive(Debug, Default)]
struct OpenAiSseStreamParser {
    buffer: Vec<u8>,
    accumulator: OpenAiStreamAccumulator,
}

impl OpenAiSseStreamParser {
    fn push_bytes(
        &mut self,
        bytes: &[u8],
        prompt_tokens: u32,
    ) -> Result<Option<OpenAiResponse>, OpenAiCompatError> {
        self.buffer.extend_from_slice(bytes);
        while let Some(event) = drain_next_sse_event(&mut self.buffer) {
            let data = parse_sse_event_bytes(event)?;
            let Some(data) = data else {
                continue;
            };
            if data.trim() == "[DONE]" {
                let accumulator = std::mem::take(&mut self.accumulator);
                return Ok(Some(accumulator.into_response(prompt_tokens)?));
            }
            apply_stream_data(&mut self.accumulator, &data)?;
        }

        Ok(None)
    }

    fn finish(self, prompt_tokens: u32) -> Result<OpenAiResponse, OpenAiCompatError> {
        self.accumulator.into_response(prompt_tokens)
    }
}

fn parse_openai_sse_events(
    raw: &str,
    prompt_tokens: u32,
) -> Result<OpenAiResponse, OpenAiCompatError> {
    let mut parser = OpenAiSseStreamParser::default();
    if let Some(response) = parser.push_bytes(raw.as_bytes(), prompt_tokens)? {
        return Ok(response);
    }

    tracing::debug!("OpenAI stream ended without explicit [DONE] marker");
    parser.finish(prompt_tokens)
}

fn drain_next_sse_event(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    let delimiter = find_sse_delimiter(buffer)?;
    let event = buffer[..delimiter.start].to_vec();
    buffer.drain(..delimiter.end);
    Some(event)
}

struct SseDelimiter {
    start: usize,
    end: usize,
}

fn find_sse_delimiter(buffer: &[u8]) -> Option<SseDelimiter> {
    let lf = buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|start| SseDelimiter {
            start,
            end: start + 2,
        });
    let crlf = buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|start| SseDelimiter {
            start,
            end: start + 4,
        });

    match (lf, crlf) {
        (Some(lf), Some(crlf)) => {
            if lf.start < crlf.start {
                Some(lf)
            } else {
                Some(crlf)
            }
        }
        (Some(lf), None) => Some(lf),
        (None, Some(crlf)) => Some(crlf),
        (None, None) => None,
    }
}

fn parse_sse_event_bytes(event: Vec<u8>) -> Result<Option<String>, OpenAiCompatError> {
    let event = String::from_utf8(event).map_err(|e| {
        OpenAiCompatError::ApiError(
            reqwest::StatusCode::OK,
            format!("Streaming UTF-8 decode error: {}", e),
        )
    })?;
    Ok(sse_event_data(&event))
}

fn sse_event_data(event: &str) -> Option<String> {
    let data_lines: Vec<String> = event
        .lines()
        .filter_map(|line| {
            let line = line.trim_end_matches('\r');
            if line.starts_with(':') {
                return None;
            }
            line.strip_prefix("data:")
                .map(|data| data.strip_prefix(' ').unwrap_or(data).to_string())
        })
        .collect();

    if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n"))
    }
}

fn apply_stream_data(
    accumulator: &mut OpenAiStreamAccumulator,
    data: &str,
) -> Result<(), OpenAiCompatError> {
    if let Ok(error) = serde_json::from_str::<OpenAiStreamError>(data) {
        return Err(stream_error_to_openai_error(error));
    }

    let chunk: OpenAiStreamChunk = serde_json::from_str(data).map_err(|e| {
        OpenAiCompatError::ApiError(
            reqwest::StatusCode::OK,
            format!("Streaming parse error: {}", e),
        )
    })?;

    if let Some(usage) = chunk.usage {
        accumulator.usage = Some(usage);
    }

    for choice in chunk.choices {
        if let Some(role) = choice.delta.role {
            accumulator.role = Some(role);
        }
        if let Some(content) = choice.delta.content {
            accumulator.content.push_str(&content);
        }
        if let Some(tool_calls) = choice.delta.tool_calls {
            for tool_call in tool_calls {
                accumulator.apply_tool_call_delta(tool_call);
            }
        }
        if let Some(reason) = choice.finish_reason {
            accumulator.finish_reason = Some(reason);
        }
    }

    Ok(())
}

fn stream_error_to_openai_error(error: OpenAiStreamError) -> OpenAiCompatError {
    let message = error
        .error
        .message
        .unwrap_or_else(|| "OpenAI stream returned an error".to_string());
    let error_type = error.error.error_type.unwrap_or_default();
    let code = error
        .error
        .code
        .map(|value| value.to_string())
        .unwrap_or_default();
    let detail = if code.is_empty() {
        message
    } else {
        format!("{} ({})", message, code)
    };

    if error_type.contains("rate_limit") || code.contains("429") {
        OpenAiCompatError::RateLimitExceeded(Duration::from_secs(60))
    } else if error_type.contains("server")
        || error_type.contains("overload")
        || code.contains("bad_gateway")
        || code.contains("timeout")
    {
        OpenAiCompatError::TransientError(Duration::from_secs(30), detail)
    } else {
        OpenAiCompatError::ApiError(reqwest::StatusCode::OK, detail)
    }
}

impl OpenAiStreamAccumulator {
    fn apply_tool_call_delta(&mut self, delta: OpenAiToolCallDelta) {
        let entry = self.tool_calls.entry(delta.index).or_default();
        if let Some(id) = delta.id {
            entry.id.get_or_insert(id);
        }
        if let Some(tool_type) = delta.tool_type {
            entry.tool_type.get_or_insert(tool_type);
        }
        if let Some(function) = delta.function {
            if let Some(name) = function.name {
                entry.name.get_or_insert(name);
            }
            if let Some(arguments) = function.arguments {
                entry.arguments.push_str(&arguments);
            }
        }
    }

    fn into_response(self, prompt_tokens: u32) -> Result<OpenAiResponse, OpenAiCompatError> {
        let has_content = !self.content.is_empty();
        let has_tool_calls = !self.tool_calls.is_empty();
        if !has_content && !has_tool_calls {
            return Err(OpenAiCompatError::ApiError(
                reqwest::StatusCode::OK,
                "OpenAI stream completed without content or tool calls".to_string(),
            ));
        }

        let tool_calls = if has_tool_calls {
            Some(
                self.tool_calls
                    .into_iter()
                    .map(|(index, call)| OpenAiToolCall {
                        id: call.id.unwrap_or_else(|| format!("call_{}", index)),
                        tool_type: call.tool_type.unwrap_or_else(|| "function".to_string()),
                        function: OpenAiToolCallFunction {
                            name: call.name.unwrap_or_default(),
                            arguments: call.arguments,
                        },
                    })
                    .collect(),
            )
        } else {
            None
        };

        let content = if has_content {
            Some(self.content)
        } else {
            None
        };
        let usage = self.usage.unwrap_or_else(|| {
            fallback_stream_usage(prompt_tokens, content.as_deref(), tool_calls.as_ref())
        });
        let finish_reason = self.finish_reason.unwrap_or_else(|| {
            if tool_calls.is_some() {
                "tool_calls".to_string()
            } else {
                "stop".to_string()
            }
        });

        Ok(OpenAiResponse {
            choices: vec![OpenAiChoice {
                index: 0,
                message: OpenAiMessage {
                    role: self.role.unwrap_or_else(|| "assistant".to_string()),
                    content,
                    tool_calls,
                    tool_call_id: None,
                },
                finish_reason,
            }],
            usage,
        })
    }
}

fn fallback_stream_usage(
    prompt_tokens: u32,
    content: Option<&str>,
    tool_calls: Option<&Vec<OpenAiToolCall>>,
) -> OpenAiUsage {
    let content_tokens = content.map(TokenBudget::estimate_tokens).unwrap_or(0);
    let tool_tokens = tool_calls
        .map(|calls| {
            calls
                .iter()
                .map(|call| {
                    TokenBudget::estimate_tokens(&call.function.name)
                        + TokenBudget::estimate_tokens(&call.function.arguments)
                })
                .sum::<usize>()
        })
        .unwrap_or(0);
    let completion_tokens = (content_tokens + tool_tokens) as u32;

    OpenAiUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
    }
}

fn translate_ai_request(
    request: AiRequest,
    max_tokens: u32,
    provider_type: OpenAiProviderType,
) -> Result<OpenAiRequest> {
    let mut messages = Vec::new();

    if let Some(system_text) = request.system {
        messages.push(OpenAiMessage {
            role: "system".to_string(),
            content: Some(system_text),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    for msg in request.messages {
        match msg.role {
            AiRole::System => {
                messages.push(OpenAiMessage {
                    role: "system".to_string(),
                    content: msg.content,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            AiRole::User => {
                messages.push(OpenAiMessage {
                    role: "user".to_string(),
                    content: msg.content,
                    tool_calls: None,
                    tool_call_id: None,
                });
            }
            AiRole::Assistant => {
                messages.push(OpenAiMessage {
                    role: "assistant".to_string(),
                    content: msg.content,
                    tool_calls: msg.tool_calls.map(|tc| {
                        tc.into_iter()
                            .map(|t| OpenAiToolCall {
                                id: t.id,
                                tool_type: "function".to_string(),
                                function: OpenAiToolCallFunction {
                                    name: t.function_name,
                                    arguments: serde_json::to_string(&t.arguments).unwrap(),
                                },
                            })
                            .collect()
                    }),
                    tool_call_id: None,
                });
            }
            AiRole::Tool => {
                messages.push(OpenAiMessage {
                    role: "tool".to_string(),
                    content: msg.content,
                    tool_calls: None,
                    tool_call_id: msg.tool_call_id,
                });
            }
        }
    }

    let tools = request.tools.and_then(|t| {
        if t.is_empty() {
            None
        } else {
            Some(
                t.into_iter()
                    .map(|tool| OpenAiTool {
                        tool_type: "function".to_string(),
                        function: OpenAiFunction {
                            name: tool.name,
                            description: tool.description,
                            parameters: tool.parameters,
                        },
                    })
                    .collect(),
            )
        }
    });

    let response_format = request.response_format.map(|rf| match rf {
        AiResponseFormat::Json { .. } => serde_json::json!({"type": "json_object"}),
        AiResponseFormat::Text => serde_json::json!({"type": "text"}),
    });

    // OpenAI requires the word "json" to appear in at least one message when
    // using response_format: json_object. Inject it if missing.
    if response_format
        .as_ref()
        .is_some_and(|rf| rf["type"] == "json_object")
    {
        let has_json = messages.iter().any(|m| {
            m.content
                .as_ref()
                .is_some_and(|c| c.to_lowercase().contains("json"))
        });
        if !has_json {
            if let Some(system_msg) = messages.iter_mut().find(|m| m.role == "system") {
                let content = system_msg.content.get_or_insert_default();
                content.push_str("\nRespond in JSON format.");
            } else {
                messages.insert(
                    0,
                    OpenAiMessage {
                        role: "system".to_string(),
                        content: Some("Respond in JSON format.".to_string()),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                );
            }
        }
    }

    let (max_tokens_field, max_completion_tokens_field) = match provider_type {
        OpenAiProviderType::OpenAi => (None, Some(max_tokens)),
        OpenAiProviderType::OpenAiCompatible => (Some(max_tokens), None),
    };

    Ok(OpenAiRequest {
        model: String::new(),
        messages,
        tools,
        temperature: request.temperature,
        max_tokens: max_tokens_field,
        max_completion_tokens: max_completion_tokens_field,
        response_format,
        stream: None,
        stream_options: None,
    })
}

fn translate_ai_response(resp: OpenAiResponse) -> Result<AiResponse> {
    let choice = resp
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No choices in response"))?;

    let content = choice.message.content;
    let tool_calls = choice.message.tool_calls.map(|tc| {
        tc.into_iter()
            .map(|t| {
                let arguments: Value =
                    serde_json::from_str(&t.function.arguments).unwrap_or(serde_json::Value::Null);
                ToolCall {
                    id: t.id,
                    function_name: t.function.name,
                    arguments,
                    thought_signature: None,
                }
            })
            .collect()
    });

    let usage = Some(AiUsage {
        prompt_tokens: resp.usage.prompt_tokens as usize,
        completion_tokens: resp.usage.completion_tokens as usize,
        total_tokens: resp.usage.total_tokens as usize,
        cached_tokens: None,
    });

    Ok(AiResponse {
        content,
        thought: None,
        thought_signature: None,
        tool_calls,
        usage,
    })
}

fn estimate_tokens_generic(request: &AiRequest) -> usize {
    let mut total = 0;
    if let Some(system) = &request.system {
        total += TokenBudget::estimate_tokens(system);
    }
    for msg in &request.messages {
        if let Some(content) = &msg.content {
            total += TokenBudget::estimate_tokens(content);
        }
        if let Some(tool_calls) = &msg.tool_calls {
            for call in tool_calls {
                total += TokenBudget::estimate_tokens(&call.function_name);
                total += TokenBudget::estimate_tokens(&call.arguments.to_string());
            }
        }
    }
    if let Some(tools) = &request.tools {
        for tool in tools {
            total += TokenBudget::estimate_tokens(&tool.name);
            total += TokenBudget::estimate_tokens(&tool.description);
            total += TokenBudget::estimate_tokens(&tool.parameters.to_string());
        }
    }
    total
}

fn is_unsupported_stream_options_error(message: &str) -> bool {
    let normalized = message.to_lowercase();
    normalized.contains("stream_options")
        && (normalized.contains("include_usage")
            || normalized.contains("unsupported")
            || normalized.contains("unknown field")
            || normalized.contains("unknown parameter")
            || normalized.contains("unrecognized"))
}

#[async_trait]
impl AiProvider for OpenAiCompatClient {
    async fn generate_content(&self, request: AiRequest) -> Result<AiResponse> {
        tracing::info!("Sending OpenAI request...");

        let prompt_tokens = estimate_tokens_generic(&request) as u32;
        let mut openai_req = translate_ai_request(request, self.max_tokens, self.provider_type)?;
        openai_req.model = self.model.clone();

        let resp = if self.streaming {
            openai_req.stream = Some(true);
            openai_req.stream_options = Some(serde_json::json!({"include_usage": true}));
            let resp_body = serde_json::to_value(&openai_req)?;
            match self.post_stream_request(&resp_body, prompt_tokens).await {
                Ok(resp) => resp,
                Err(OpenAiCompatError::ApiError(status, message))
                    if status == reqwest::StatusCode::BAD_REQUEST
                        && is_unsupported_stream_options_error(&message) =>
                {
                    openai_req.stream_options = None;
                    let retry_body = serde_json::to_value(&openai_req)?;
                    self.post_stream_request(&retry_body, prompt_tokens).await?
                }
                Err(err) => return Err(err.into()),
            }
        } else {
            let resp_body = serde_json::to_value(&openai_req)?;
            self.post_request(&resp_body).await?
        };
        translate_ai_response(resp)
    }

    fn estimate_tokens(&self, request: &AiRequest) -> usize {
        estimate_tokens_generic(request)
    }

    fn get_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            model_name: self.model.clone(),
            context_window_size: self.context_window_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{AiErrorClass, AiMessage, AiTool, ClassifyAiError, DEFAULT_RETRY_AFTER};
    use serde_json::json;

    #[test]
    fn test_rate_limit_exceeded_classifies_as_rate_limit() {
        let retry_after = Duration::from_secs(7);
        let err = OpenAiCompatError::RateLimitExceeded(retry_after);

        assert_eq!(
            err.ai_error_class(),
            AiErrorClass::RateLimit { retry_after }
        );
    }

    #[test]
    fn test_transient_error_classifies_as_transient() {
        let retry_after = Duration::from_secs(11);
        let err = OpenAiCompatError::TransientError(retry_after, "busy".to_string());

        assert_eq!(
            err.ai_error_class(),
            AiErrorClass::Transient { retry_after }
        );
    }

    #[test]
    fn test_authentication_error_classifies_as_fatal() {
        let err = OpenAiCompatError::AuthenticationError("bad key".to_string());

        assert_eq!(err.ai_error_class(), AiErrorClass::Fatal);
    }

    #[test]
    fn test_api_error_server_status_classifies_as_transient() {
        let err = OpenAiCompatError::ApiError(
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
            "unavailable".to_string(),
        );

        assert_eq!(
            err.ai_error_class(),
            AiErrorClass::Transient {
                retry_after: DEFAULT_RETRY_AFTER,
            }
        );
    }

    #[test]
    fn test_api_error_client_status_classifies_as_fatal() {
        let err = OpenAiCompatError::ApiError(
            reqwest::StatusCode::BAD_REQUEST,
            "bad request".to_string(),
        );

        assert_eq!(err.ai_error_class(), AiErrorClass::Fatal);
    }

    #[test]
    fn test_translate_request_system_and_user() -> Result<()> {
        let request = AiRequest {
            system: Some("You are helpful.".to_string()),
            messages: vec![AiMessage {
                role: AiRole::User,
                content: Some("Hello!".to_string()),
                thought: None,
                thought_signature: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: Some(0.7),
            response_format: None,
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        assert_eq!(openai_req.messages.len(), 2);
        assert_eq!(openai_req.messages[0].role, "system");
        assert_eq!(
            openai_req.messages[0].content,
            Some("You are helpful.".to_string())
        );
        assert_eq!(openai_req.messages[1].role, "user");
        assert_eq!(openai_req.messages[1].content, Some("Hello!".to_string()));
        assert_eq!(openai_req.temperature, Some(0.7));
        assert_eq!(openai_req.max_tokens, Some(4096));

        Ok(())
    }

    #[test]
    fn test_translate_request_system_in_messages() -> Result<()> {
        let request = AiRequest {
            system: None,
            messages: vec![
                AiMessage {
                    role: AiRole::System,
                    content: Some("Be concise.".to_string()),
                    thought: None,
                    thought_signature: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                AiMessage {
                    role: AiRole::User,
                    content: Some("Say hi.".to_string()),
                    thought: None,
                    thought_signature: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
            ],
            tools: None,
            temperature: None,
            response_format: None,
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        assert_eq!(openai_req.messages.len(), 2);
        assert_eq!(openai_req.messages[0].role, "system");
        assert_eq!(
            openai_req.messages[0].content,
            Some("Be concise.".to_string())
        );
        assert_eq!(openai_req.messages[1].role, "user");

        Ok(())
    }

    #[test]
    fn test_translate_request_assistant_tool_call() -> Result<()> {
        let request = AiRequest {
            system: None,
            messages: vec![AiMessage {
                role: AiRole::Assistant,
                content: Some("I'll use a tool.".to_string()),
                thought: None,
                thought_signature: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call_123".to_string(),
                    function_name: "test_tool".to_string(),
                    arguments: json!({"arg1": "val1"}),
                    thought_signature: None,
                }]),
                tool_call_id: None,
            }],
            tools: None,
            temperature: None,
            response_format: None,
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        assert_eq!(openai_req.messages.len(), 1);
        assert_eq!(openai_req.messages[0].role, "assistant");
        assert_eq!(
            openai_req.messages[0].content,
            Some("I'll use a tool.".to_string())
        );
        let tool_calls = openai_req.messages[0].tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].function.name, "test_tool");
        assert_eq!(tool_calls[0].function.arguments, r#"{"arg1":"val1"}"#);
        assert_eq!(tool_calls[0].tool_type, "function");

        Ok(())
    }

    #[test]
    fn test_translate_request_tool_response() -> Result<()> {
        let request = AiRequest {
            system: None,
            messages: vec![AiMessage {
                role: AiRole::Tool,
                content: Some(json!({"result": "success"}).to_string()),
                thought: None,
                thought_signature: None,
                tool_calls: None,
                tool_call_id: Some("call_123".to_string()),
            }],
            tools: None,
            temperature: None,
            response_format: None,
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        assert_eq!(openai_req.messages.len(), 1);
        assert_eq!(openai_req.messages[0].role, "tool");
        assert_eq!(
            openai_req.messages[0].tool_call_id,
            Some("call_123".to_string())
        );
        assert_eq!(
            openai_req.messages[0].content,
            Some(r#"{"result":"success"}"#.to_string())
        );

        Ok(())
    }

    #[test]
    fn test_translate_request_tools_definition() -> Result<()> {
        let request = AiRequest {
            system: None,
            messages: vec![],
            tools: Some(vec![AiTool {
                name: "my_tool".to_string(),
                description: "Does something.".to_string(),
                parameters: json!({"type": "object"}),
            }]),
            temperature: None,
            response_format: None,
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        let tools = openai_req.tools.as_ref().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].tool_type, "function");
        assert_eq!(tools[0].function.name, "my_tool");
        assert_eq!(tools[0].function.description, "Does something.");
        assert_eq!(tools[0].function.parameters, json!({"type": "object"}));

        Ok(())
    }

    #[test]
    fn test_translate_request_empty_tools() -> Result<()> {
        let request = AiRequest {
            system: None,
            messages: vec![],
            tools: Some(vec![]),
            temperature: None,
            response_format: None,
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        // An empty tools array should be mapped to None so it gets skipped in serialization
        assert!(openai_req.tools.is_none());

        Ok(())
    }

    #[test]
    fn test_translate_request_conversation_chain() -> Result<()> {
        let request = AiRequest {
            system: None,
            messages: vec![
                AiMessage {
                    role: AiRole::User,
                    content: Some("Use tool".to_string()),
                    thought: None,
                    thought_signature: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                AiMessage {
                    role: AiRole::Assistant,
                    content: None,
                    thought: None,
                    thought_signature: None,
                    tool_calls: Some(vec![ToolCall {
                        id: "c1".to_string(),
                        function_name: "t1".to_string(),
                        arguments: json!({}),
                        thought_signature: None,
                    }]),
                    tool_call_id: None,
                },
                AiMessage {
                    role: AiRole::Tool,
                    content: Some(r#"{"ok":true}"#.to_string()),
                    thought: None,
                    thought_signature: None,
                    tool_calls: None,
                    tool_call_id: Some("c1".to_string()),
                },
            ],
            tools: Some(vec![AiTool {
                name: "t1".to_string(),
                description: "d1".to_string(),
                parameters: json!({}),
            }]),
            temperature: None,
            response_format: None,
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        assert_eq!(openai_req.messages.len(), 3);
        assert_eq!(openai_req.messages[0].role, "user");
        assert_eq!(openai_req.messages[1].role, "assistant");
        assert_eq!(openai_req.messages[2].role, "tool");
        assert_eq!(openai_req.messages[2].tool_call_id.as_deref(), Some("c1"));
        assert!(openai_req.tools.is_some());

        Ok(())
    }

    #[test]
    fn test_translate_request_json_format() -> Result<()> {
        let schema = json!({
            "type": "object",
            "properties": {"score": {"type": "number"}}
        });
        let request = AiRequest {
            system: None,
            messages: vec![AiMessage {
                role: AiRole::User,
                content: Some("Score this.".to_string()),
                thought: None,
                thought_signature: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: None,
            response_format: Some(AiResponseFormat::Json {
                schema: Some(schema.clone()),
            }),
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        assert_eq!(
            openai_req.response_format,
            Some(json!({"type": "json_object"}))
        );
        // "json" not in any message, so a system message should be prepended
        assert_eq!(openai_req.messages[0].role, "system");
        assert_eq!(
            openai_req.messages[0].content,
            Some("Respond in JSON format.".to_string())
        );
        assert_eq!(openai_req.messages.len(), 2);

        Ok(())
    }

    #[test]
    fn test_translate_request_json_format_no_injection_when_present() -> Result<()> {
        let request = AiRequest {
            system: Some("You are helpful.".to_string()),
            messages: vec![AiMessage {
                role: AiRole::User,
                content: Some("Return the score as JSON.".to_string()),
                thought: None,
                thought_signature: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: None,
            response_format: Some(AiResponseFormat::Json { schema: None }),
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        assert_eq!(
            openai_req.response_format,
            Some(json!({"type": "json_object"}))
        );
        // "json" already in user message, system prompt should be unchanged
        assert_eq!(openai_req.messages.len(), 2);
        assert_eq!(
            openai_req.messages[0].content,
            Some("You are helpful.".to_string())
        );

        Ok(())
    }

    #[test]
    fn test_translate_request_temperature() -> Result<()> {
        let request = AiRequest {
            system: None,
            messages: vec![AiMessage {
                role: AiRole::User,
                content: Some("Test".to_string()),
                thought: None,
                thought_signature: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: Some(0.5),
            response_format: None,
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        assert_eq!(openai_req.temperature, Some(0.5));

        Ok(())
    }

    #[test]
    fn test_translate_response_text() -> Result<()> {
        let openai_resp = OpenAiResponse {
            choices: vec![OpenAiChoice {
                index: 0,
                message: OpenAiMessage {
                    role: "assistant".to_string(),
                    content: Some("Hello!".to_string()),
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: "stop".to_string(),
            }],
            usage: OpenAiUsage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            },
        };

        let ai_resp = translate_ai_response(openai_resp)?;

        assert_eq!(ai_resp.content, Some("Hello!".to_string()));
        assert_eq!(ai_resp.thought, None);
        assert_eq!(ai_resp.tool_calls, None);
        let usage = ai_resp.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
        assert_eq!(usage.cached_tokens, None);

        Ok(())
    }

    #[test]
    fn test_translate_response_tool_calls() -> Result<()> {
        let openai_resp = OpenAiResponse {
            choices: vec![OpenAiChoice {
                index: 0,
                message: OpenAiMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![OpenAiToolCall {
                        id: "call_abc".to_string(),
                        tool_type: "function".to_string(),
                        function: OpenAiToolCallFunction {
                            name: "my_tool".to_string(),
                            arguments: r#"{"arg":"val"}"#.to_string(),
                        },
                    }]),
                    tool_call_id: None,
                },
                finish_reason: "tool_calls".to_string(),
            }],
            usage: OpenAiUsage {
                prompt_tokens: 15,
                completion_tokens: 25,
                total_tokens: 40,
            },
        };

        let ai_resp = translate_ai_response(openai_resp)?;

        assert_eq!(ai_resp.content, None);
        assert_eq!(ai_resp.thought, None);
        let tool_calls = ai_resp.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_abc");
        assert_eq!(tool_calls[0].function_name, "my_tool");
        assert_eq!(tool_calls[0].arguments["arg"], "val");
        assert_eq!(tool_calls[0].thought_signature, None);

        Ok(())
    }

    #[test]
    fn test_translate_response_empty_choices() {
        let openai_resp = OpenAiResponse {
            choices: vec![],
            usage: OpenAiUsage {
                prompt_tokens: 10,
                completion_tokens: 0,
                total_tokens: 10,
            },
        };

        let result = translate_ai_response(openai_resp);
        assert!(result.is_err());
    }

    #[test]
    fn test_estimate_tokens() {
        let request = AiRequest {
            system: Some("System prompt".to_string()),
            messages: vec![
                AiMessage {
                    role: AiRole::User,
                    content: Some("Short message".to_string()),
                    thought: None,
                    thought_signature: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                AiMessage {
                    role: AiRole::Assistant,
                    content: None,
                    thought: None,
                    thought_signature: None,
                    tool_calls: Some(vec![ToolCall {
                        id: "c1".to_string(),
                        function_name: "my_function".to_string(),
                        arguments: json!({"key": "value"}),
                        thought_signature: None,
                    }]),
                    tool_call_id: None,
                },
            ],
            tools: Some(vec![AiTool {
                name: "my_function".to_string(),
                description: "Does something".to_string(),
                parameters: json!({"type": "object"}),
            }]),
            temperature: None,
            response_format: None,
            context_tag: None,
        };

        let tokens = estimate_tokens_generic(&request);
        assert!(tokens > 10);
        assert!(tokens < 200);
    }

    #[test]
    fn test_max_tokens_for_openai_compatible() -> Result<()> {
        let request = AiRequest {
            system: None,
            messages: vec![AiMessage {
                role: AiRole::User,
                content: Some("Test".to_string()),
                thought: None,
                thought_signature: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: None,
            response_format: None,
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        assert_eq!(openai_req.max_tokens, Some(4096));
        assert_eq!(openai_req.max_completion_tokens, None);

        // Verify serialized JSON has max_tokens and no max_completion_tokens
        let json = serde_json::to_value(&openai_req)?;
        assert_eq!(json["max_tokens"], 4096);
        assert!(json.get("max_completion_tokens").is_none());

        Ok(())
    }

    #[test]
    fn test_max_completion_tokens_for_openai() -> Result<()> {
        let request = AiRequest {
            system: None,
            messages: vec![AiMessage {
                role: AiRole::User,
                content: Some("Test".to_string()),
                thought: None,
                thought_signature: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: None,
            response_format: None,
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAi)?;

        assert_eq!(openai_req.max_tokens, None);
        assert_eq!(openai_req.max_completion_tokens, Some(4096));

        // Verify serialized JSON has max_completion_tokens and no max_tokens
        let json = serde_json::to_value(&openai_req)?;
        assert!(json.get("max_tokens").is_none());
        assert_eq!(json["max_completion_tokens"], 4096);

        Ok(())
    }

    #[test]
    fn test_streaming_request_serializes_stream_fields() -> Result<()> {
        let request = AiRequest {
            system: None,
            messages: vec![AiMessage {
                role: AiRole::User,
                content: Some("Test".to_string()),
                thought: None,
                thought_signature: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            tools: None,
            temperature: None,
            response_format: None,
            context_tag: None,
        };

        let mut openai_req =
            translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;
        openai_req.stream = Some(true);
        openai_req.stream_options = Some(json!({"include_usage": true}));

        let json = serde_json::to_value(&openai_req)?;

        assert_eq!(json["stream"], true);
        assert_eq!(json["stream_options"]["include_usage"], true);

        Ok(())
    }

    #[test]
    fn test_parse_openai_sse_events_accumulates_content_and_usage_fallback() -> Result<()> {
        let raw = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\" world\"},\"finish_reason\":null}]}\n\n",
            "data: [DONE]\n\n"
        );

        let response = parse_openai_sse_events(raw, 12)?;

        assert_eq!(response.choices.len(), 1);
        assert_eq!(
            response.choices[0].message.content.as_deref(),
            Some("Hello world")
        );
        assert_eq!(response.choices[0].finish_reason, "stop");
        assert_eq!(response.usage.prompt_tokens, 12);
        assert!(response.usage.completion_tokens > 0);
        assert_eq!(
            response.usage.total_tokens,
            response.usage.prompt_tokens + response.usage.completion_tokens
        );

        Ok(())
    }

    #[test]
    fn test_parse_openai_sse_events_uses_stream_usage() -> Result<()> {
        let raw = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":7,\"completion_tokens\":3,\"total_tokens\":10}}\n\n",
            "data: [DONE]\n\n"
        );

        let response = parse_openai_sse_events(raw, 99)?;

        assert_eq!(response.usage.prompt_tokens, 7);
        assert_eq!(response.usage.completion_tokens, 3);
        assert_eq!(response.usage.total_tokens, 10);

        Ok(())
    }

    #[test]
    fn test_parse_openai_sse_events_accumulates_tool_call_deltas() -> Result<()> {
        let raw = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_x\",\"type\":\"function\",\"function\":{\"name\":\"git_log\",\"arguments\":\"{\\\"\"}}]},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"limit\\\":10}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n"
        );

        let response = parse_openai_sse_events(raw, 5)?;

        let tool_calls = response.choices[0].message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_x");
        assert_eq!(tool_calls[0].tool_type, "function");
        assert_eq!(tool_calls[0].function.name, "git_log");
        assert_eq!(tool_calls[0].function.arguments, r#"{"limit":10}"#);
        assert_eq!(response.choices[0].finish_reason, "tool_calls");

        Ok(())
    }

    #[test]
    fn test_parse_openai_sse_events_returns_error_for_stream_error() {
        let raw = "data: {\"error\":{\"message\":\"stream blocked\",\"type\":\"server_error\",\"code\":\"bad_gateway\"}}\n\n";

        let result = parse_openai_sse_events(raw, 5);

        assert!(matches!(
            result,
            Err(OpenAiCompatError::TransientError(_, message)) if message.contains("stream blocked")
        ));
    }

    #[test]
    fn test_stream_parser_returns_response_when_done_arrives() -> Result<()> {
        let raw = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"done\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ignored\"},\"finish_reason\":null}]}\n\n"
        );
        let mut parser = OpenAiSseStreamParser::default();

        let response = parser.push_bytes(raw.as_bytes(), 3)?.unwrap();

        assert_eq!(response.choices[0].message.content.as_deref(), Some("done"));
        Ok(())
    }

    #[test]
    fn test_stream_parser_preserves_utf8_split_across_chunks() -> Result<()> {
        let event = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"café\"},\"finish_reason\":\"stop\"}]}\n\n";
        let split_at = event
            .as_bytes()
            .windows(2)
            .position(|window| window == "é".as_bytes())
            .unwrap()
            + 1;
        let mut parser = OpenAiSseStreamParser::default();

        assert!(
            parser
                .push_bytes(&event.as_bytes()[..split_at], 3)?
                .is_none()
        );
        let done = "data: [DONE]\n\n";
        let second = [&event.as_bytes()[split_at..], done.as_bytes()].concat();
        let response = parser.push_bytes(&second, 3)?.unwrap();

        assert_eq!(response.choices[0].message.content.as_deref(), Some("café"));
        Ok(())
    }

    #[test]
    fn test_translate_request_preserves_tool_schemas() -> Result<()> {
        let request = AiRequest {
            system: None,
            messages: vec![],
            tools: Some(vec![AiTool {
                name: "my_tool".to_string(),
                description: "Does something.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "mode": { "type": "string" }
                    }
                }),
            }]),
            temperature: None,
            response_format: None,
            context_tag: None,
        };

        let openai_req = translate_ai_request(request, 4096, OpenAiProviderType::OpenAiCompatible)?;

        let tools = openai_req.tools.as_ref().unwrap();
        assert_eq!(tools[0].function.parameters["type"], "object");
        assert_eq!(
            tools[0].function.parameters["properties"]["mode"]["type"],
            "string"
        );

        Ok(())
    }

    #[test]
    fn test_normalize_base_url_appends_chat_completions() {
        // LM Studio style: just /v1
        assert_eq!(
            OpenAiCompatClient::normalize_base_url("http://localhost:1234/v1"),
            "http://localhost:1234/v1/chat/completions"
        );
        // Trailing slash
        assert_eq!(
            OpenAiCompatClient::normalize_base_url("http://localhost:1234/v1/"),
            "http://localhost:1234/v1/chat/completions"
        );
        // Already has full path
        assert_eq!(
            OpenAiCompatClient::normalize_base_url("https://api.openai.com/v1/chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
        // Full path with trailing slash
        assert_eq!(
            OpenAiCompatClient::normalize_base_url("http://localhost:1234/v1/chat/completions/"),
            "http://localhost:1234/v1/chat/completions"
        );
        // Bare host
        assert_eq!(
            OpenAiCompatClient::normalize_base_url("http://localhost:1234"),
            "http://localhost:1234/chat/completions"
        );
    }
}
