# Design: Explicit Gemini Caching for Review Prompts

## 1. Objective
Leverage Gemini's [Context Caching](https://ai.google.dev/gemini-api/docs/caching) feature to significantly reduce latency and operational costs for `sashiko` code reviews. By caching a comprehensive "Reviewer Knowledge Base", we avoid resending static instruction sets (prompts, patterns, documentation) for every patch review.

## 2. Problem Statement
-   **Latency**: Sending large prompt contexts (system instructions + prompt library) increases Time to First Token (TTFT).
-   **Cost**: Repeatedly sending the same input tokens costs more than using cached tokens.
-   **Token Threshold**: Gemini Caching requires a minimum of **32,768 tokens**. Individual prompt files (e.g., `review-core.md` ~8KB) are too small to be cached individually.
-   **Context Fragmentation**: Currently, we dynamically select *subsets* of prompts (e.g., only `net` + `bpf`) to save context space, potentially missing cross-subsystem insights.

## 3. Solution: The "Universal Reviewer" Cache
Instead of caching small, specific contexts, we will build and cache a **single, comprehensive context** that contains the entire universe of review rules and relevant kernel documentation.

### 3.1. Cache Content Composition
The cached content (approx. 140k - 150k tokens) will be structured as follows:

1.  **System Persona**:
    -   Content from `review-prompts/review-core.md` (Identity, tone, core philosophy).
2.  **Kernel Standards (The "Constitution")**:
    -   `linux/Documentation/process/coding-style.rst`
    -   `linux/Documentation/process/submitting-patches.rst`
    -   `linux/Documentation/process/submit-checklist.rst`
    -   *Why*: Provides ground truth for style and process, adding "heft" to reach the 32k limit and improving model accuracy.
3.  **Prompt Library (The "Knowledge Base")**:
    -   Iterate through `review-prompts/*.md` (excluding `review-core.md` used above).
    -   Iterate through `review-prompts/patterns/*.md`.
    -   Format:
        ```markdown
        ## Subsystem Guidelines: Networking (`networking.md`)
        [Content...]

        ## Subsystem Guidelines: Memory Management (`mm.md`)
        [Content...]
        ```
    -   *Why*: Gives the model access to *all* subsystem rules. The model is intelligent enough to apply only the rules relevant to the files modified in the user's patch.

### 3.2. Lifecycle Management
-   **TTL**: Set to **60 minutes** (default). Refreshed automatically on use.
-   **Key Generation**:
    -   Compute a standard SHA-256 hash of all input files (prompts + docs).
    -   Cache Name: `sashiko-reviewer-v1-<HASH_PREFIX>` (e.g., `sashiko-reviewer-v1-a1b2c3d4`).
-   **Invalidation**:
    -   If the local file hash differs from the active cache's hash (implied by name), create a new cache.
    -   Old caches expire naturally.

## 4. Architectural Changes

### 4.1. `GeminiClient` Updates (`src/ai/gemini.rs`)
The client needs support for the Caching API.

-   **New Structs**:
    -   `CachedContent` (Model for the API resource).
    -   `CreateCachedContentRequest`.
-   **New Methods**:
    -   `create_cached_content(request: CreateCachedContentRequest) -> Result<CachedContent>`
    -   `get_cached_content(name: &str) -> Result<CachedContent>`
    -   `generate_content_with_cache(cache_name: &str, request: GenerateContentRequest)`
        -   *Note*: When using cache, the `model` parameter in the URL typically stays the same, but the request body refers to the cache resource.

### 4.2. `CacheManager` (`src/ai/cache.rs`)
A new module to handle the logic:
1.  **`build_context()`**: Reads files from disk and concatenates them into the prompt structure defined in 3.1.
2.  **`ensure_cache()`**:
    -   Calculates hash of `build_context()` output.
    -   List existing caches (via API `GET /v1beta/cachedContents`).
    -   If a match exists: return its resource name.
    -   If no match: call `create_cached_content` and return the new name.

### 4.3. Integration in `Agent`
-   On startup (or first review), the Agent initializes the `CacheManager`.
-   The `CacheManager` ensures the "Universal Reviewer" cache exists.
-   The Agent sends review requests referencing the `cachedContent` resource name instead of sending the `system_instruction` text.

## 5. Usage Flow

1.  **Startup**:
    -   `sashiko` calculates hash of `review-prompts/` + `Documentation/`.
    -   Checks Gemini API for cache named `...-<HASH>`.
2.  **Cache Miss**:
    -   Uploads the ~500KB context to Gemini.
    -   Receives `cachedContents/12345...`.
3.  **Review Request**:
    -   User: "Review this patch..."
    -   Payload:
        ```json
        {
          "cachedContent": "cachedContents/12345...",
          "contents": [ { "role": "user", "parts": [{ "text": "Patch content..." }] } ]
        }
        ```
    -   System Prompt is *implicit* in the cache.

## 6. Benefits
-   **Fixed Cost**: We pay for the cache storage (cheap) + 1 write. Reads are significantly cheaper than input tokens.
-   **Speed**: Caching eliminates the processing time for the system prompt on every request.
-   **Simplicity**: Removing the "Dynamic Prompt Selector" logic makes the agent code more robust and deterministic.

## 7. Safety & Limits
-   **Token Limit**: Gemini 1.5 Flash/Pro has 1M/2M context. 150k is well within safety margins.
-   **Minimum**: We safely exceed the 32k minimum.
-   **Fallback**: If caching API fails (quota/error), fall back to the old method of sending `review-core.md` + specific subsystem prompts (the "Lite" version).
