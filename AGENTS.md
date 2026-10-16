# Working agreement for coding agents (Codex / Claude)

Communication between agents and the owner happens **through commits and the plans in
`docs/superpowers/plans/`**. There is no other channel.

## Branches (who works where)
* `main` is protected by convention: **never push to it directly.**
* One plan = one branch named `phase2/<letter>-<name>` (e.g. `phase2/a-item-layer`), created from
  the latest `main`. Push after every task so the owner and the reviewing agent see progress.
* When the plan's last task is done and CI is green, open a pull request into `main`.
* Plans are executed in order A -> D. Do not start D before A is merged. Plan B (except B3)
  and Plan C are on hold, replaced by the circles design
  (`docs/superpowers/specs/2026-10-13-circles-design.md`); do not execute them.
* Only one agent works on a given plan at a time. Claude reviews the commits/PR; it does not
  push to a plan branch that Codex is working on.

## Workflow
1. Read `docs/superpowers/specs/2026-10-13-phase-2-design.md`, then the plan you were pointed at.
2. Execute the plan **task by task, in order**. One task = one commit (or a few small ones).
3. Before every commit run, from the repo root (Windows, MSVC toolchain):
   ```powershell
   cargo fmt
   cargo clippy --release --locked -- -D warnings
   cargo test --release --locked
   ```
   CI runs `cargo fmt --check` first; an unformatted commit turns CI red.
4. Commit message: imperative summary, a short body saying *why*, and
   `Plan: <plan file> Task <n>` as the last line. If a plan step could not be done as
   written, say so in the body and describe what you did instead - do not silently diverge.
5. Mark finished steps `- [x]` in the plan file in the same commit.
6. Do not run `scripts/windows-integration-test.ps1` unless a task asks for it: it moves the
   mouse, presses Win+D and writes to the user's Desktop. It is for a human-supervised machine.

## Hard rules
* Never modify/move/delete the user's files; items are references only.
* Do not hold `RefCell` borrows across message-pumping calls (see the design doc).
* `panic = "abort"`: a panic is a crash. No `unwrap()` on Win32/shell results.
* No new dependencies unless the plan names them.
* Windows-only code: build/test with the MSVC target on Windows. Wine results are not evidence.

## Reading context without loading the whole repository

* Start with `docs/superpowers/plans/CONTEXT.md` and Git status/branch/worktrees.
  Use the checkout that owns the requested work; the startup checkout may be older.
* Status questions need the short context, relevant plan evidence and live Git/CI,
  not every historical design/code sample.
* For implementation, read the required design and active task once, then only
  relevant sections. Workflow step 1 does not require dumping an entire multi-task
  plan on every turn. Keep the safety and acceptance constraints in context.
* Use bounded searches/reads as described in CONTEXT.md. Do not read build outputs,
  test captures or scratch scripts unless the task needs them.
* Keep CONTEXT.md under 100 lines and update changed facts in the same commit.
  Label observed defects, proposed fixes and unperformed checks separately.
