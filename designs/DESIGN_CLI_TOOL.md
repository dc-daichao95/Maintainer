# Sashiko CLI Tool Design

## Objective
Create a command-line interface (`sashiko-cli`) to interact with the Sashiko instance. This tool will allow users to submit patches, check status, and manage reviews from the terminal, complementing the web UI.

## Architecture
The CLI will be a standalone binary (`src/bin/sashiko-cli.rs`) that communicates with the Sashiko server via its HTTP API. It will share the `Settings` configuration to locate the server (host/port).

## Configuration
The CLI will load `Settings.toml` to determine the server URL.
- `server.host`: Target host (default: 127.0.0.1)
- `server.port`: Target port (default: 8080)

## Commands

### 1. `submit`
Submits patches for review.

**Usage:**
```bash
sashiko-cli submit [OPTIONS] <INPUT>
```

**Options:**
- `--type <TYPE>`: Submission type.
  - `mbox`: Raw mbox content (file or stdin).
  - `remote`: Single remote commit hash.
  - `range`: Remote commit range (e.g., `origin/main..HEAD`).
- `--repo <PATH>`: Repository path (for remote/range types). Defaults to current directory.
- `--baseline <COMMIT>`: Baseline commit (for mbox injection).

**Examples:**
```bash
# Submit local range
sashiko-cli submit --type range origin/master..HEAD

# Submit single commit
sashiko-cli submit --type remote deadbeef

# Submit mbox file
sashiko-cli submit --type mbox patch.mbox

# Submit from stdin
cat patch.mbox | sashiko-cli submit --type mbox -
```

### 2. `status`
Displays server status and statistics.

**Usage:**
```bash
sashiko-cli status
```

**Output:**
- Server version/health.
- Queue counts (pending, reviewing, failed).
- Database stats (messages, patchsets).

### 3. `list`
Lists patchsets or reviews.

**Usage:**
```bash
sashiko-cli list [OPTIONS]
```

**Options:**
- `--limit <N>`: Number of items.
- `--page <N>`: Page number.
- `--filter <STATUS>`: Filter by status (pending, reviewed, error).

### 4. `show`
Shows details of a specific patchset or review.

**Usage:**
```bash
sashiko-cli show <ID>
```

## UI/UX
- **Colors:** Use `termcolor` to highlight status (Green for success, Red for failure, Yellow for pending).
- **Output:** Clean, formatted text. JSON output optional via `--json`.
- **Progress:** Simple spinners for long-running API calls (if applicable).

## Implementation Plan
1.  **Dependencies:** Use `clap` for args, `reqwest` for API, `serde` for JSON, `termcolor` for styling.
2.  **Binary:** Create `src/bin/sashiko-cli.rs`.
3.  **Shared Code:** Reuse `Settings` and `api` structs (move common API types to `lib.rs` if needed, currently they are in `api.rs` which is part of the library).
4.  **Execution:** Implement commands one by one.

## Future Extensions
- `watch`: Monitor a patchset status in real-time.
- `config`: View/Edit configuration.
