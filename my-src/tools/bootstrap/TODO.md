# One-Click Deployment Tool Checklist

## Phase 1: Scaffolding
- [x] Create `requirements.txt` with `tomlkit`
- [x] Create `config_template.json`
- [x] Create `README.md`

## Phase 2: Implementation (`deploy.py`)
- [x] Implement config loading and validation
- [x] Implement Rust installation placeholder execution
- [x] Implement `git clone` logic
- [x] Implement kernel symlink management (`third_party/linux`)
- [x] Implement `Settings.toml` modification using `tomlkit`
- [x] Implement `.env` file generation for `LLM_API_KEY`
- [ ] Implement robust error handling and logging

## Phase 3: Testing
- [x] Write unit tests for config parsing
- [x] Write unit tests for TOML modification
- [x] Write unit tests for command execution
- [x] Perform E2E test in a mock environment

## Phase 4: Finalization
- [x] Finalize `README.md`
- [x] Code cleanup and documentation
