# new UI
- [ ] 支持后台服务配置 - 检视分支、检视范围（按MR、按commit）
- [ ] 支持人工触发检视，个人检视时必须使用个人api key
- [ ] 支持检视问题视图，可以按人、按模块、按MR、按问题等级不同维度来查询
- [ ] 支持配置通知功能：群号、按问题@责任人、定期发送报告（要能够真正地@）

# 检视规则
- [ ] 整合内存prompt，并验证
- [ ] 基于2025年问题准备内核检视bench mark
- [ ] 收集历史关键问题
- [ ] 基于历史问题提供检视bench mark

# Spec-00002: Code Base Management Sync Pipeline

## Phase 1: Foundation & Data Models (TDD)
- [x] **Task 1.1**: Implement `RepositoryMapping` and `SyncStatus` serialization tests.
- [x] **Task 1.2**: Add validation logic for `SyncPipelineRequest`.

## Phase 2: GitEngine & Low-level Operations (TDD)
- [x] **Task 2.1**: Define `GitProvider` trait to allow mocking of system commands.
- [x] **Task 2.2**: Implement `extract_baseline`.
- [x] **Task 2.3**: Implement `execute_script` for `apply-patches`.
- [x] **Task 2.4**: Implement `git_diff`, `apply_patch_to_target`, and `commit_changes`.

## Phase 3: Repository Managers (TDD)
- [x] **Task 3.1**: `SourceRepoMgr::get_latest_mr`.
- [x] **Task 3.2**: `SourceRepoMgr::revert_repo_state`.
- [x] **Task 3.3**: `TargetRepoMgr::prepare_target_branch`.

## Phase 4: SyncController Orchestration (TDD)
- [x] **Task 4.1**: Implement the 8-step logic in `SyncController::execute_sync`.
- [x] **Task 4.2**: Implement State Machine transitions.
- [x] **Task 4.3**: Implement Error Handling (No-Retry Policy).

## Phase 5: CLI Tool & E2E Testing
- [x] **Task 5.1**: Implement `my-src/tools/sync_cli.rs`.
- [x] **Task 5.2**: Create `my-src/tests/sync_cli_e2e_test.rs`.