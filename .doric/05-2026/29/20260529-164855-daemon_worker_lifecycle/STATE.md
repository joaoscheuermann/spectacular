# State

## Cursor

- Phase: development
- Current effort: none
- Next effort index: 2

## Approvals

| Gate                            | Approved | Evidence |
| ------------------------------- | -------- | -------- |
| tdd_to_decomposition            | yes      | agents/019_approval_tdd_to_decomposition.md |
| decomposition_to_implementation | yes      | agents/023_approval_decomposition_to_implementation.md |

## Effort order

| Index | Effort file | Status |
| ----- | ----------- | ------ |
| 0 | efforts/01_workspace_package_skeletons.md | done |
| 1 | efforts/02_lifecycle_proto_codegen.md | done |
| 2 | efforts/03_lifecycle_domain_redaction.md | todo |
| 3 | efforts/04_cli_lifecycle_parse_routing.md | todo |
| 4 | efforts/05_daemon_registry_root.md | todo |
| 5 | efforts/06_daemon_lifecycle_service.md | todo |
| 6 | efforts/07_daemon_process_worker_session.md | todo |
| 7 | efforts/08_worker_repo_preparation.md | todo |
| 8 | efforts/09_worker_provider_runtime.md | todo |
| 9 | efforts/10_worker_tooling_registration.md | todo |
| 10 | efforts/11_worker_prompt_agent_runner.md | todo |
| 11 | efforts/12_worker_session_runtime.md | todo |
| 12 | efforts/13_cli_daemon_client_output.md | todo |
| 13 | efforts/14_lifecycle_integration_smoke.md | todo |
| 14 | efforts/15_architecture_docs_validation.md | todo |

## Active locks

| Effort | Owner | Write scope | Status |
| ------ | ----- | ----------- | ------ |
| none | none | none | none |

## Required agents

| Phase  | Effort | Role                  | Agent type | Receipt                                    | Agent id                             | Status   |
| ------ | ------ | --------------------- | ---------- | ------------------------------------------ | ------------------------------------ | -------- |
| prompt | none   | requirements elicitor | explorer   | agents/001_prompt_requirements_elicitor.md | 019e7548-ca28-7ab0-b429-19de229c9237 | accepted |
| prd    | none   | PRD writer            | worker     | agents/002_prd_writer.md                   | 019e754a-5546-73a3-9755-7078957050e4 | accepted |
| prd    | none   | PRD initial filter evaluator | explorer | agents/003_prd_initial_filter_evaluator.md | 019e754d-9f65-7c23-90ee-e734268d6bea | accepted |
| prd    | none   | PRD grounded evaluator | explorer | agents/004_prd_grounded_evaluator.md | 019e754f-13ac-71a2-8ee5-d2bbae8a28fe | accepted |
| prd    | none   | PRD assumption evaluator | explorer | agents/005_prd_assumption_evaluator.md | 019e7550-e835-77f2-81bf-ba0188267cc6 | accepted |
| technical_design | none | technical design author | worker | agents/006_technical_design_author.md | 019e7553-8354-7e43-9ee2-de8688b76c53 | accepted |
| technical_design | none | technical initial filter evaluator | explorer | agents/007_technical_initial_filter_evaluator.md | 019e755a-9ef3-7962-869c-9fb78195e2ff | accepted |
| technical_design | none | technical grounded evaluator | explorer | agents/008_technical_grounded_evaluator.md | 019e755c-bb72-7013-aa16-996ed59376d9 | accepted |
| technical_design | none | technical design gap repair author | worker | agents/009_technical_gap_repair_author.md | 019e7561-0243-7ed1-aa30-e5da76911300 | accepted |
| technical_design | none | technical initial filter evaluator epoch 2 | explorer | agents/010_technical_initial_filter_evaluator_epoch_2.md | 019e7565-95bb-7492-8ec1-116dac9eb68d | accepted |
| technical_design | none | technical design dependency repair author | worker | agents/011_technical_dependency_repair_author.md | 019e7569-2e4a-7da3-b968-80927a99b044 | accepted |
| technical_design | none | technical initial filter evaluator epoch 3 | explorer | agents/012_technical_initial_filter_evaluator_epoch_3.md | 019e756c-a448-7562-870a-81da0a86762c | accepted |
| technical_design | none | technical grounded evaluator epoch 3 | explorer | agents/013_technical_grounded_evaluator_epoch_3.md | 019e756f-204b-7aa3-a933-0c232011a004 | accepted |
| technical_design | none | technical provider-runtime repair author | worker | agents/014_technical_provider_runtime_repair_author.md | 019e7573-bff8-7a70-9345-9c523dafe75b | accepted |
| technical_design | none | technical grounded evaluator epoch 4 | explorer | agents/015_technical_grounded_evaluator_epoch_4.md | 019e7578-7e00-7dd0-a251-a4d1d2378eec | accepted |
| technical_design | none | technical tool-scope repair author | worker | agents/016_technical_tool_scope_repair_author.md | 019e757d-27e5-7080-af4c-062cd6d497c1 | accepted |
| technical_design | none | technical grounded evaluator epoch 5 | explorer | agents/017_technical_grounded_evaluator_epoch_5.md | 019e7581-443f-73b3-9314-8f63c60a1e07 | accepted |
| technical_design | none | technical assumption evaluator epoch 5 | explorer | agents/018_technical_assumption_evaluator_epoch_5.md | 019e7585-a7d2-7473-a450-e0313e12a698 | accepted |
| decomposition | none | requirement extractor | worker | agents/020_decomposition_requirement_extractor.md | 019e7604-e58e-75e2-91db-291ddf86262b | accepted |
| decomposition | none | decomposition validator | explorer | agents/021_decomposition_validator.md | 019e7607-c963-71e3-83b4-1c2148bb7fca | accepted |
| decomposition | none | effort planner | worker | agents/022_decomposition_effort_planner.md | 019e7609-8ee6-74e0-bc76-c5f38e851987 | accepted |
| development | efforts/01_workspace_package_skeletons.md | test planner | worker | agents/025_effort_01_test_planner.md | 019e7639-6d34-70e1-acfa-edcb12f6e9a8 | accepted |
| development | efforts/01_workspace_package_skeletons.md | test writer | worker | agents/026_effort_01_test_writer.md | 019e763b-6095-7743-92bc-bf239abb26c5 | accepted |
| development | efforts/01_workspace_package_skeletons.md | code writer | worker | agents/027_effort_01_code_writer.md | 019e763d-99f9-73e1-b01d-3de671883850 | accepted |
| development | efforts/01_workspace_package_skeletons.md | validator/refactor | worker | agents/028_effort_01_validator_refactor.md | 019e7641-9a93-7843-9d88-5cb80b3442d9 | accepted |
| development | efforts/01_workspace_package_skeletons.md | reviewer | explorer | agents/029_effort_01_reviewer.md | 019e7644-516e-7f43-bbb1-4d1fb0ebbaf7 | superseded |
| development | efforts/01_workspace_package_skeletons.md | reviewer | explorer | agents/030_effort_01_reviewer_checkpoint_retry.md | 019e7647-2285-70d0-b442-e0ad56561b21 | accepted |
| development | efforts/02_lifecycle_proto_codegen.md | test planner | worker | agents/033_effort_02_test_planner.md | 019e764b-192c-7b23-933a-6126db89abd9 | accepted |
| development | efforts/02_lifecycle_proto_codegen.md | test writer | worker | agents/034_effort_02_test_writer.md | 019e764d-16d3-7830-8f54-54c35b77165f | accepted |
| development | efforts/02_lifecycle_proto_codegen.md | code writer | worker | agents/035_effort_02_code_writer.md | 019e7650-2ceb-79e3-8d36-c85b06e2aea5 | accepted |
| development | efforts/02_lifecycle_proto_codegen.md | validator/refactor | worker | agents/036_effort_02_validator_refactor.md | 019e7655-0ddc-7b01-a335-2f44a3a1115e | accepted |
| development | efforts/02_lifecycle_proto_codegen.md | reviewer | explorer | agents/037_effort_02_reviewer.md | 019e7657-8521-7d22-bdf7-0b7079e3cb34 | superseded |
| development | efforts/02_lifecycle_proto_codegen.md | reviewer checkpoint retry | explorer | agents/039_effort_02_reviewer_checkpoint_retry.md | 019e765c-91a2-7c12-a818-e2da17c43104 | accepted |

## Agent receipts

| Receipt                                    | Phase  | Effort | Role                  | Agent type | Agent id                             | Status   |
| ------------------------------------------ | ------ | ------ | --------------------- | ---------- | ------------------------------------ | -------- |
| agents/001_prompt_requirements_elicitor.md | prompt | none   | requirements elicitor | explorer   | 019e7548-ca28-7ab0-b429-19de229c9237 | accepted |
| agents/002_prd_writer.md                   | prd    | none   | PRD writer            | worker     | 019e754a-5546-73a3-9755-7078957050e4 | accepted |
| agents/003_prd_initial_filter_evaluator.md | prd    | none   | PRD initial filter evaluator | explorer | 019e754d-9f65-7c23-90ee-e734268d6bea | accepted |
| agents/004_prd_grounded_evaluator.md       | prd    | none   | PRD grounded evaluator | explorer  | 019e754f-13ac-71a2-8ee5-d2bbae8a28fe | accepted |
| agents/005_prd_assumption_evaluator.md     | prd    | none   | PRD assumption evaluator | explorer | 019e7550-e835-77f2-81bf-ba0188267cc6 | accepted |
| agents/006_technical_design_author.md      | technical_design | none | technical design author | worker | 019e7553-8354-7e43-9ee2-de8688b76c53 | accepted |
| agents/007_technical_initial_filter_evaluator.md | technical_design | none | technical initial filter evaluator | explorer | 019e755a-9ef3-7962-869c-9fb78195e2ff | accepted |
| agents/008_technical_grounded_evaluator.md | technical_design | none | technical grounded evaluator | explorer | 019e755c-bb72-7013-aa16-996ed59376d9 | accepted |
| agents/009_technical_gap_repair_author.md | technical_design | none | technical design gap repair author | worker | 019e7561-0243-7ed1-aa30-e5da76911300 | accepted |
| agents/010_technical_initial_filter_evaluator_epoch_2.md | technical_design | none | technical initial filter evaluator epoch 2 | explorer | 019e7565-95bb-7492-8ec1-116dac9eb68d | accepted |
| agents/011_technical_dependency_repair_author.md | technical_design | none | technical design dependency repair author | worker | 019e7569-2e4a-7da3-b968-80927a99b044 | accepted |
| agents/012_technical_initial_filter_evaluator_epoch_3.md | technical_design | none | technical initial filter evaluator epoch 3 | explorer | 019e756c-a448-7562-870a-81da0a86762c | accepted |
| agents/013_technical_grounded_evaluator_epoch_3.md | technical_design | none | technical grounded evaluator epoch 3 | explorer | 019e756f-204b-7aa3-a933-0c232011a004 | accepted |
| agents/014_technical_provider_runtime_repair_author.md | technical_design | none | technical provider-runtime repair author | worker | 019e7573-bff8-7a70-9345-9c523dafe75b | accepted |
| agents/015_technical_grounded_evaluator_epoch_4.md | technical_design | none | technical grounded evaluator epoch 4 | explorer | 019e7578-7e00-7dd0-a251-a4d1d2378eec | accepted |
| agents/016_technical_tool_scope_repair_author.md | technical_design | none | technical tool-scope repair author | worker | 019e757d-27e5-7080-af4c-062cd6d497c1 | accepted |
| agents/017_technical_grounded_evaluator_epoch_5.md | technical_design | none | technical grounded evaluator epoch 5 | explorer | 019e7581-443f-73b3-9314-8f63c60a1e07 | accepted |
| agents/018_technical_assumption_evaluator_epoch_5.md | technical_design | none | technical assumption evaluator epoch 5 | explorer | 019e7585-a7d2-7473-a450-e0313e12a698 | accepted |
| agents/020_decomposition_requirement_extractor.md | decomposition | none | requirement extractor | worker | 019e7604-e58e-75e2-91db-291ddf86262b | accepted |
| agents/021_decomposition_validator.md | decomposition | none | decomposition validator | explorer | 019e7607-c963-71e3-83b4-1c2148bb7fca | accepted |
| agents/022_decomposition_effort_planner.md | decomposition | none | effort planner | worker | 019e7609-8ee6-74e0-bc76-c5f38e851987 | accepted |
| agents/025_effort_01_test_planner.md | development | efforts/01_workspace_package_skeletons.md | test planner | worker | 019e7639-6d34-70e1-acfa-edcb12f6e9a8 | accepted |
| agents/026_effort_01_test_writer.md | development | efforts/01_workspace_package_skeletons.md | test writer | worker | 019e763b-6095-7743-92bc-bf239abb26c5 | accepted |
| agents/027_effort_01_code_writer.md | development | efforts/01_workspace_package_skeletons.md | code writer | worker | 019e763d-99f9-73e1-b01d-3de671883850 | accepted |
| agents/028_effort_01_validator_refactor.md | development | efforts/01_workspace_package_skeletons.md | validator/refactor | worker | 019e7641-9a93-7843-9d88-5cb80b3442d9 | accepted |
| agents/029_effort_01_reviewer.md | development | efforts/01_workspace_package_skeletons.md | reviewer | explorer | 019e7644-516e-7f43-bbb1-4d1fb0ebbaf7 | superseded |
| agents/030_effort_01_reviewer_checkpoint_retry.md | development | efforts/01_workspace_package_skeletons.md | reviewer | explorer | 019e7647-2285-70d0-b442-e0ad56561b21 | accepted |
| agents/033_effort_02_test_planner.md | development | efforts/02_lifecycle_proto_codegen.md | test planner | worker | 019e764b-192c-7b23-933a-6126db89abd9 | accepted |
| agents/034_effort_02_test_writer.md | development | efforts/02_lifecycle_proto_codegen.md | test writer | worker | 019e764d-16d3-7830-8f54-54c35b77165f | accepted |
| agents/035_effort_02_code_writer.md | development | efforts/02_lifecycle_proto_codegen.md | code writer | worker | 019e7650-2ceb-79e3-8d36-c85b06e2aea5 | accepted |
| agents/036_effort_02_validator_refactor.md | development | efforts/02_lifecycle_proto_codegen.md | validator/refactor | worker | 019e7655-0ddc-7b01-a335-2f44a3a1115e | accepted |
| agents/037_effort_02_reviewer.md | development | efforts/02_lifecycle_proto_codegen.md | reviewer | explorer | 019e7657-8521-7d22-bdf7-0b7079e3cb34 | superseded |
| agents/039_effort_02_reviewer_checkpoint_retry.md | development | efforts/02_lifecycle_proto_codegen.md | reviewer checkpoint retry | explorer | 019e765c-91a2-7c12-a818-e2da17c43104 | accepted |

## Validation records

| Effort | Record | Red | Green | Reviewer |
| ------ | ------ | --- | ----- | -------- |
| efforts/01_workspace_package_skeletons.md | validation/01_workspace_package_skeletons.md | yes | yes | approved |
| efforts/02_lifecycle_proto_codegen.md | validation/02_lifecycle_proto_codegen.md | yes | yes | approved |

## Commit checkpoints

| Effort | Commit | Message | Staged scope |
| ------ | ------ | ------- | ------------ |
| efforts/01_workspace_package_skeletons.md | 261e0a7f4cfd598b4ce15eb2e463124559b0b5a6 | chore(workspace): add lifecycle daemon worker package skeletons | `git commit --only -- Cargo.toml Cargo.lock packages/lifecycle packages/daemon packages/worker .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle` |
| efforts/02_lifecycle_proto_codegen.md | pending | feat(lifecycle): add proto codegen contract | `git commit --only -- Cargo.lock packages/lifecycle/Cargo.toml packages/lifecycle/build.rs packages/lifecycle/proto/doric/lifecycle/v1.proto packages/lifecycle/src/lib.rs packages/lifecycle/src/proto.rs packages/lifecycle/tests/unit/proto_contract.rs .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/STATE.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/031_effort_01_done_transition.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/032_effort_02_start_transition.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/033_effort_02_test_planner.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/034_effort_02_test_writer.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/035_effort_02_code_writer.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/036_effort_02_validator_refactor.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/037_effort_02_reviewer.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/038_effort_02_checkpoint_scope_reconciliation.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/039_effort_02_reviewer_checkpoint_retry.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/agents/040_effort_02_done_transition.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/efforts/02_lifecycle_proto_codegen.md .doric/05-2026/29/20260529-164855-daemon_worker_lifecycle/validation/02_lifecycle_proto_codegen.md` |
