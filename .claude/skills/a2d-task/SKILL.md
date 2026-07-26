---
name: a2d-task
description: Implement one task from docs/A2D_SMART_NOTEBOOK_V01_TODO.md end to end under this repo's completion bar, then commit it with the task ID. Invoked as /a2d-task <task-id>, e.g. /a2d-task 3.2.
disable-model-invocation: true
---

Implement TODO task **$ARGUMENTS** for the A2D Smart Notebook.

This repo's global `/ralph-loop` skill carries rules from an unrelated ESP32/Bluetooth project — ignore those and follow this file instead.

## 1. Read before writing

- Find the task in `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` by its ID. Read the whole surrounding milestone, including its Acceptance list.
- Read the matching section of `docs/A2D_SMART_NOTEBOOK_V01_SPEC.md`. MUST/MUST NOT there are requirements.
- If the spec and TODO conflict, the spec wins — say so in your response rather than picking silently.

## 2. Implement

Follow the architecture rules in `CLAUDE.md`: Rust owns domain logic, SQL stays in `a2d-storage`, `a2d-ffi` stays thin, opaque newtype IDs, the `A2dError` envelope on every fallible path, no error-erasing conversions, no panics across FFI.

Scope to the task. Do not implement adjacent TODO items, and do not write speculative code for milestones that haven't started.

If an undecided item blocks you (toolchain version, UniFFI mode, min Android API, crate choice, threshold value): pick a sensible default, **state the assumption explicitly**, and record it in the relevant doc. Never invent a measured threshold — those must come from testing.

## 3. Completion bar

The task is **not** done if any of these is true:

- A mock or stub stands in for the completed path
- Error handling is implicit or a failure is swallowed into `None` / empty / `false`
- A test was made to pass by editing a golden fixture under `fixtures/`
- The acceptance behavior listed in the TODO has not been demonstrated
- The only evidence of success is "it didn't crash"

Run the narrowest relevant tests as you work. Before committing, run `/a2d-check` and report the result honestly — including gates that are not yet wired up.

## 4. Commit

- Tick the task's checkbox in the TODO file in the same commit.
- Append a short note to `memory.md` at the repo root: what changed, what was decided, what is still open.
- One commit for this task, on `master`, message includes the task ID:
  `feat(storage): 3.2 add asset commit protocol`
- **No `Co-Authored-By:` trailer** — a global `commit-msg` hook rejects it.
- Push after committing.

Ask before committing if the working tree already contained unrelated changes when you started.
