### 1. Prompt (Requirements Elicitation & Memory Bank)
* **Goal:** Run an interactive loop to help the user clarify what the feature is, why it needs to be implemented, and address any ambiguity.
* **Execution Details & Improvements:**
  * **Interactive Elicitation Loop:** The agent uses an active-listening prompting framework (e.g., asking clarifying questions, presenting hypothetical edge cases) to draw out the core user value and commercial driver.
  * **Persistent Memory Bank:** Once aligned, the agent compiles this conversation into a highly structured `PROMPT.md` file (stored in a dedicated `/context` directory in your file system). 
  * **Role in the Workflow:** This file serves as the "source of truth" and acts as long-term RAM for all subsequent agent steps, preventing context drift as the chat history grows.

### 2. PRD (Product Requirements Document Generation)
* **Goal:** Ask the user questions to flesh out product needs, then run a multi-agent loop to generate and refine a high-fidelity PRD.
* **Execution Details & Improvements:**
  * **Initial Questionnaire:** The system asks a brief, targeted set of questions focusing on user personas, acceptance criteria, and out-of-scope elements.
  * **The PM-Evaluator Loop:** Spawn an autonomous loop containing two distinct agents:
    * **`product manager` Agent:** Drafts and updates the PRD based on the `PROMPT.md` and user responses.
    * **`evaluator` Agent:** Analyzes the draft against an objective, non-negotiable rubric (checking for ambiguous requirements, missing edge cases, and compliance gaps).
  * **Multi-Epoch Optimization:** This generator-critic loop iterates over a set number of epochs ($N$).
    * *Improvement:* To prevent the two agents from agreeing on flawed assumptions (local minima), the evaluator must use a hard-coded constraint list.
    * *Improvement:* If the evaluator finds structural gaps that cannot be resolved autonomously, the loop pauses to ask the user for clarification.
  * **Artifact Persistence:** Save the finalized PRD as `PRD.md` in the file system, capturing metadata such as target KPIs and strict out-of-scope boundaries.

### 3. TDD / Architecture (Technical Design Document Generation)
* **Goal:** Ask a few questions about overall architectural direction, then run an Architect-Evaluator loop with context handovers to generate a technical blueprint.
* **Execution Details & Improvements:**
  * **Tech Direction Input:** Gather initial constraints from the user (e.g., preferred database, API standards, libraries, or structural patterns).
  * **The Architect-Evaluator Loop:**
    * **`architect` Agent:** Proposes the database schema, API contracts, design patterns, and system components.
    * **`evaluator` Agent:** Evaluates the proposed technical design against security standards, database normalization rules, potential performance bottlenecks, and compatibility with the existing codebase.
  * **Context Handover Pattern:** 
    * *Improvement:* To avoid token bloat and LLM memory decay, when the `evaluator` identifies gaps, it compiles a structured markdown report. 
    * *Improvement:* Rather than continuing in the same thread, a **brand-new, stateless `architect` instance** is spawned. It receives the previous TDD draft and the evaluator’s report to perform the edits with a clean memory.
  * **Human-in-the-Loop Gate:** If the evaluator finds missing domain information, it prompts the user.
  * **User Approval Checkpoint:** Once $N$ epochs are exhausted, the finalized `TDD.md` is presented to the user. The system halts and requests explicit permission (e.g., "Type 'approve' to proceed") before entering the decomposition phase.

### 4. Decomposition (Hierarchical Task Decomposition)
* **Goal:** Break down the architecture into discrete, trackable technical documents (efforts) for incremental implementation.
* **Execution Details & Improvements:**
  * **Feature Extraction & Verification:** 
    * Agent A lists all the features described in the `PRD.md` and `TDD.md`.
    * Agent B (serving as a validator) cross-references this list against the original documents to ensure zero features were missed or altered.
  * **Decomposition into "Efforts":** 
    * For each feature, the system breaks it down into incremental steps called "efforts." Each effort is focused strictly on making small, isolated modifications to the codebase.
    * *Improvement:* Implement **Graph-Based Impact Analysis** here. The decomposition agent identifies not just what code to write, but which existing files are coupled to it and which existing test suites must be run to prevent regressions.
  * **Tracking State on Disk:** Save each effort as an individual markdown file (e.g., `/efforts/01_add_auth_route.md`) containing:
    * Current status (`todo`, `in-progress`, `done`).
    * Targeted files to modify.
    * Specific acceptance criteria and test cases.

### 5. Development (Test-Driven Agentic Development)
* **Goal:** Execute a highly focused Red-Green-Refactor cycle for each effort using a specialized squad of coding agents.
* **Execution Details & Improvements:**
  * **Step A: Test Breakdown:** Before any source code is touched, an agent analyzes the current effort file and breaks the task down into a list of programmatic tests.
  * **Step B: The Red-Green-Refactor Team:** The tests and the codebase are handed over to three highly specialized sub-agents:
    * **`test writer` Agent:** Writes the unit or integration tests based on the spec. *Improvement:* This agent cannot write implementation code, preventing it from tailoring tests to fit a lazy code implementation (mitigating "specification laundering").
    * **`code writer` Agent:** Implements the actual application logic to make the tests pass. It is instructed to strictly adhere to the codebase's existing style guidelines.
    * **`validator/refactor` Agent:** Runs the test suites. If they pass, it reviews the code for quality, performance, and formatting, planning refactoring steps if necessary.
  * **The Development Loop & Guardrails:**
    * *Improvement (Circuit Breaker):* The `code writer` and `validator` run in a loop to fix failing tests. If the tests do not pass within a maximum limit (e.g., 5 attempts), the loop breaks, aborts the run, and escalates the issue to the user.
  * **Step C: Post-Implementation Review:** 
    * Once tests are green, a separate reviewer agent compares the changes against the original goal and scope of the effort to ensure no scope creep or unrelated refactoring occurred.
    * The agent runs the broader system test suite (determined during Phase 4's impact analysis) to confirm no regressions were introduced.
  * **Step D: Atomic Commit / Rollback Checkpoint:**
    * If all tests and reviews pass, the status of the effort file is updated on disk to `done`, and the changes are committed to the repository.
    * *Improvement (Transactional Integrity):* If the effort fails or is interrupted in an incomplete (`progress`) state, the system executes an atomic rollback (e.g., `git reset --hard`) to return the workspace to the last green, stable commit. When restarted, the agent begins the effort from a clean state.

### 6. Handover (Notification of Completion)
* **Goal:** Safely notify the user that the entire feature has been successfully completed and verified.
* **Execution Details & Improvements:**
  * **Handover Report:** The system generates a summary notice showing:
    * A list of all completed efforts.
    * A summary of the generated test suites that passed.
    * A list of modified files.
  * **User Prompt:** The system informs the user that the task is done and prompts them for their next instruction.
