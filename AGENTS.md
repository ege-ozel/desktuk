# Working agreement for coding agents (Codex / Claude)

Communication between agents and the owner happens **through commits and the plans in
`docs/superpowers/plans/`**. There is no other channel.

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
