# ADR-00001: 采用主进程伪流式（Pseudo-Streaming）解决大模型 API 网关超时问题

## 状态 (Status)
已提议 (Proposed) / 接受 (Accepted)

## 编者 (Author)
Sashiko Agent

## 1. 背景与问题 (Context & Problem)
在当前系统架构中，Sashiko 默认采用**非流式（Non-Streaming）**调用来请求各大语言模型（LLM）服务。当模型进行长文本生成或复杂推理时，生成时间通常会达到几十秒甚至一两分钟。
由于在此期间客户端与服务端之间没有数据传输，导致经常触发 API 供应商网关（如 Nginx、API Gateway）的空闲超时限制（通常为 60 秒），进而抛出 `504 Gateway Timeout` 错误。

供应商强制建议改用**流式调用（Streaming / SSE）**，通过持续的数据分块传输来打断网关的超时倒计时机制。

然而，Sashiko 目前的架构对流式支持存在以下障碍：
1. **多进程 IPC 架构限制**：Sashiko 采用主进程（负责配额与网络）与 Review Worker 子进程通过标准输入输出（stdin/stdout）进行一次性的一问一答 JSON 通信。
2. **AiProvider 接口设计**：底层的 `generate_content` 返回的是一个完整的 `Result<AiResponse>`，而非异步数据流。
3. **工具调用（Tool Calls）的复杂性**：流式调用下，函数名和 JSON 参数是被切碎发送的，无法直接反序列化。

## 2. 备选方案 (Considered Options)

### 方案 A：主进程伪流式聚合（Parent-Process Pseudo-Streaming）
- **机制**：在底层 HTTP Client 层（如 `openai.rs`）开启流式请求（`stream: true`）。主进程接收 SSE 流并在内存中持续拼接文本与 Tool Calls 碎片。当流结束并组装成一个完整的 `AiResponse` 后，再通过现有的 IPC 协议一次性发送给子进程。
- **优点**：完美规避网关超时；对 Worker 子进程、IPC 通信协议、数据库和前端 UI **完全透明**，无需修改核心架构。
- **缺点**：底层客户端需处理 SSE 解析与流式参数重组逻辑。

### 方案 B：端到端真流式（End-to-End True Streaming）
- **机制**：修改底层 Trait 返回 `Stream`，修改 IPC 协议（如新增 `ai_response_chunk` 事件类型），并让 Worker 子进程能够异步处理流数据。
- **优点**：架构上最符合流式设计，理论上可降低峰值内存峰刺，支持前端实时“打字机”渲染。
- **缺点**：作为后台静默代码审查工具，“打字机”效果非刚需；需要对整个系统架构（从底层到 IPC 再到 Worker 业务逻辑）进行伤筋动骨的大规模重构。

## 3. 决策 (Decision)
我们决定采用 **方案 A（主进程伪流式聚合）**。

在 Sashiko 作为一个后台自动化 Review 机器人的定位下，我们引入 Streaming 仅仅是为了解决网络网关的超时痛点，而非为了提升流式 UI 体验。方案 A 在保证系统稳定性和降低重构风险方面具有压倒性优势。

## 4. 影响与风险缓解 (Consequences & Risk Mitigation)

实施该方案后，系统将面临以下主要风险及对应的缓解措施：

1. **Token 计费与统计风险**
   - **影响**：很多 OpenAI 兼容接口在流式模式下只有最后一个 Chunk 包含 `usage` 字段，若网络闪断可能导致计费数据丢失。
   - **缓解**：如果流正常结束但未收到 `usage`，将调用系统现有的 `TokenBudget::estimate_tokens(拼接好的全文本)` 进行后备（Fallback）估算，确保调用频率与配额限制功能正常运作。

2. **错误处理的复杂化**
   - **影响**：流式 HTTP 状态码通常为 `200 OK`，但错误信息可能混杂在 SSE 数据流中（例如中途内容被拦截），导致部分成功部分失败。
   - **缓解**：必须在底层的 SSE 解析循环中增加针对 `{"error": ...}` 的特化拦截逻辑，一旦发现错误，立即中断拼接，并将此错误转换为 Rust 层的 `Result::Err` 返回给上游重试机制。

3. **Tool Calls 流式解析**
   - **影响**：工具调用参数在流中是切片到达的。
   - **缓解**：在主进程中引入一个状态机（Buffer），接收到 `tool_calls` delta 时不断将其 Append 到当前调用的参数字符串中，待该调用结束时再执行一次性的 `serde_json::from_str` 解析。

4. **系统内存占用**
   - 内存占用将与当前非流式调用持平，因为最终仍是在内存中组装了一份完整数据，不会引入新的内存泄漏风险。