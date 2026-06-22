# Design: Smart Baseline Detection

## Goal
Improve the success rate of patch application and the relevance of AI reviews by automatically detecting the correct git baseline (repository and branch) for a given patchset. Instead of defaulting to `HEAD` or `linux-next`, the system should infer the target tree (e.g., `bpf-next`, `net-next`, `drm-misc`) based on the files touched and the patch subject.

## Problem
The Linux kernel development process involves many subsystem-specific trees.
- A patch for the BPF subsystem should ideally be applied to `bpf-next/master`.
- Applying it to `torvalds/linux` (mainline) might fail if it depends on recent changes in `bpf-next`.
- Applying it to `linux-next` is generally safe but might be unstable or too broad.
- `sashiko-review` currently defaults to `HEAD`, causing "No such file" or "Patch failed" errors for subsystem-specific patches.

## Solution Architecture

We will implement a `BaselineDetector` component within the main `sashiko` process (specifically available to the `Reviewer` service). This component will:
1.  **Preload** the `MAINTAINERS` file from the kernel repository at startup.
2.  **Parse** it into an efficient in-memory structure mapping file patterns (`F:`) to Tree URLs (`T:`).
3.  **Analyze** each patchset before review to determine the best baseline.
4.  **Pass** the resolved baseline (e.g., `net-next/master`) to the `sashiko-review` tool.

### 1. `MaintainersParser` (src/baseline.rs)

Responsible for parsing the `MAINTAINERS` file.

**Structs:**
```rust
struct MaintainersEntry {
    subsystem: String,
    trees: Vec<String>, // T: git git://...
    patterns: Vec<String>, // F: drivers/net/
}

struct BaselineRegistry {
    entries: Vec<MaintainersEntry>,
    // Optional: caching/optimization structures (e.g., trie or aho-corasick for paths?)
    // Given MAINTAINERS size (~2500 entries), linear scan matching might be acceptable, 
    // or simple prefix matching optimization.
}
```

**Logic:**
- Read `MAINTAINERS` line by line.
- Group lines into entries separated by blank lines.
- Extract `T:` lines (Git trees). Ignore non-git T entries (quilt, hg, etc. - rare).
- Extract `F:` lines (File patterns).
- Store in `BaselineRegistry`.

### 2. `BaselineHeuristics`

Logic to select the best tree given a list of file paths and a subject line.

**Algorithm:**
1.  **File Matching**:
    - For each file in the patchset diff:
        - Find all `MaintainersEntry` where `F:` pattern matches the file path.
        - Collect the associated `T:` (Tree) URLs.
    - Count the frequency of each Tree URL.
    - Identify "Candidate Trees" (e.g., `{bpf-next: 5 files, net-next: 2 files}`).

2.  **Disambiguation (Subject)**:
    - If multiple Candidate Trees exist, or if a single tree has variants (e.g., `bpf` vs `bpf-next` often listed in same section or related sections):
    - Check the **Subject Line** for keywords.
    - **Keywords**:
        - `-next`, `next` -> Prefer trees ending in `-next` or `linux-next`.
        - `bpf` -> Prefer `bpf` or `bpf-next` trees.
        - `net` -> Prefer `net` or `net-next` trees.
        - `drm` -> Prefer `drm` trees.
        - `stable` -> Prefer stable trees (rare for dev reviews).
    - If Subject contains `[PATCH bpf-next]`, strictly prefer `bpf-next` tree if present in candidates.

3.  **Local Remote Mapping**:
    - The `T:` entries are full URLs (e.g., `git://git.kernel.org/.../bpf-next.git`).
    - The local git repo might assume this is a remote named `bpf-next`.
    - **Mapping Logic**:
        - List local git remotes (`git remote -v`).
        - Map `T:` URLs to local Remote Names.
        - If a match is found (e.g., URL matches `bpf-next` remote), use `remotes/bpf-next/master`.
        - If no match is found, fallback to `linux-next` or `origin/master`.

### 3. Integration with Reviewer Service

**Startup:**
- `Reviewer` service initializes `BaselineDetector`.
- `BaselineDetector` reads `MAINTAINERS` from `Settings.git.repository_path`.
- Loads local git remotes map.

**Processing Loop:**
- When picking a patchset:
    - Fetch file list (from `db.get_patch_diffs` - verify we can extract filenames from diffs cheaply or store them. Currently we store full diff. We can parse filenames from diff headers `diff --git a/path b/path`).
    - Call `detector.detect_baseline(file_paths, subject)`.
    - Result: `String` (e.g., `bpf-next/master` or `HEAD` as fallback).
    - Pass this string to `sashiko-review --baseline ...`.

## Implementation Steps

1.  **Refactor `src/baseline.rs`**: It currently exists but is likely empty or minimal. Implement `MaintainersParser`.
2.  **Git Remote Cache**: Implement logic to read `git remote -v` into a `HashMap<Url, RemoteName>`.
3.  **Diff Parser**: Helper to extract touched filenames from raw diff content (scan for `+++ b/path`).
4.  **Heuristic Logic**: Implement the voting and subject matching.
5.  **Wire Up**: Update `Reviewer::new` to load this, and `process_pending_patchsets` to use it.

## Edge Cases
- **No MAINTAINERS match**: Fallback to `linux-next` (if available) or `HEAD`.
- **Multiple disjoint trees**: (e.g. patch touches `fs/` and `net/`). Use Subject to decide. If ambiguous, prefer `linux-next` (union of all trees).
- **Remote not configured**: If `bpf-next` is detected but not added locally, we cannot use it. Fallback to `linux-next` or `HEAD`. Log a warning suggesting to add the remote.
