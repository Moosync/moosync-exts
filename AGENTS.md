# Moosync Extensions Repository Guidelines

## Overview

`moosync-exts` contains extensions for the Moosync music player. Extensions run inside sandboxed WebAssembly (WASM) runtimes (via Wasmtime), written in TypeScript/JavaScript (QuickJS), Rust, or Go.

---

## Runtime Nature & WASM Constraints

1. **Single-Threaded Sequential Execution**:
   - WASM runtimes in Moosync execute single-threaded without background threads or independent background event loops.
   - Detached listeners (e.g., event emitter `.on("token", ...)` patterns) will NOT trigger in the background once command handling returns.
   - All asynchronous work must be sequentially awaited within the lifecycle of the incoming `handle_extension_command` invocation.
2. **Host-Mediated I/O**:
   - Extensions have no direct OS access. All network requests (HTTP) and persistent storage (`set_secure`, `get_secure`, preferences) are mediated via host Protobuf messages (`ExtensionCommand` and `MainCommand`).
3. **Memory & Sandboxing**:
   - Extensions operate within strict WASM memory limits. Clean up large payloads and avoid memory leaks.

---

## Skill References

| Skill | Purpose | When to Use |
| --- | --- | --- |
| **extensions** | Guidelines and patterns for implementing extensions across all languages (Rust, Go, Python, JS/TS). | Scaffolding, architecture, provider scopes, and IPC. See `.agents/skills/extensions/SKILL.md`. |
| **extensions-js** | Authoring extensions in JavaScript/TypeScript for the QuickJS WASM runtime. | Implementing or modifying JS/TS extensions. See `.agents/skills/extensions-js/SKILL.md`. |
| **moounit** | Writing, recording, sanitizing, and replaying tests for extensions using Moounit. | Writing unit tests, recording `expectations.json`, and running test suites. See `.agents/skills/moounit/SKILL.md`. |

---

## Core Rules

1. **Always Use Bazel**: Build with `bazel build //extension_name:extension_name` and test with `bazel test //extension_name:test`. Never invoke `npm`, `cargo`, or `go` directly.
2. **Zero `any` and Zero `as` in TypeScript**: Strictly type all models and APIs using protobuf types and type guards (`item.is(...)`).
3. **No Unsanitized Expectations**: Never commit live OAuth tokens, refresh tokens, or bearer headers in `expectations.json`. Always sanitize after recording.
4. **Specific Assertions**: Always assert specific IDs, titles, URLs, durations, and account states in tests. Avoid weak or generic assertions.
