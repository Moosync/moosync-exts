---
name: moounit
description: Writing and running Moounit tests for Moosync extensions. Covers test structure, record and replay modes, volatility filtering, and credential sanitization.
---

# Moounit Testing Skill

## Overview

`moounit` is the testing framework for Moosync extensions. It executes extension WASM modules in a hermetic test harness and mocks external HTTP calls and host commands via recorded expectations (`expectations.json`).

---

## Setting Up Tests

Each extension contains:
- `test.py`: Test definitions using `pytest` and `entry: Moounit` fixture.
- `expectations.json`: Recorded HTTP interactions and host commands.
- `BUILD`: Declares `moounit_test`:
  ```python
  load("@extensions_sdk//moounit:defs.bzl", "moounit_test")

  moounit_test(
      name = "test",
      srcs = ["test.py"],
      debug = True,
      extension = ":my_extension",
  )
  ```

---

## Record vs Replay Modes

> [!IMPORTANT]
> **Never manually craft or hand-edit `expectations.json`**. Always generate expectations through record mode (`bazel run //extension_name:test_record`). Hand-written or synthetic expectations hide real runtime bugs, mask protocol mismatches, and violate test authenticity.

1. **Replay Mode (Default)**:
   - Run via `bazel test //extension_name:test`.
   - Runs hermetically offline. All external HTTP requests and host commands match against entries in `expectations.json`.
2. **Record Mode**:
   - Run to record live network requests and host commands into `expectations.json`.
   - Run once when creating new tests or updating API contracts.
   - Use test filters (e.g. `-k test_name`) to record only targeted tests without overwriting other expectations.
   - *Requires explicit user approval before running.*

---

## Test Design Guidelines

### 1. Comprehensive Coverage
Write dedicated tests for each capability implemented:
- `test_get_provider_scopes`
- `test_search`
- `test_song_from_url`
- `test_playlist_from_url`
- `test_playlist_songs`
- `test_artist_songs`
- `test_album_songs`
- `test_recommendations`
- `test_oauth_flow`

### 2. Specific Assertions
Never use weak assertions (e.g. just checking `is not None` or `len > 0`). Assert specific values:
```python
assert song_detail.song.id == "youtube:dQw4w9WgXcQ"
assert song_detail.song.title == "Never Gonna Give You Up"
assert song_detail.song.duration.seconds == 213
assert song_detail.artists[0].artist_name == "Rick Astley"
assert account.username == "Sahil Gupte"
assert account.logged_in is True
```

### 3. Testing for Success & End-to-End Functionality
Across all kinds of extensions (not just player providers), every feature implemented by the extension should be tested for success:
- Never settle for only asserting unauthorized or error paths — tests must assert that operations actually **succeed** and produce correct outcomes.
- Rely on Moounit's **record mode** (`bazel run //extension:test_record`) to automatically record external HTTP calls, WebSockets, processes, and host interactions into `expectations.json` rather than manually crafting verbose mock expectations.
- Once recorded, tests run hermetically in replay mode (`bazel test //extension:test`) asserting success responses, valid state, and specific values.
- For player providers:
  - Verify account authorization & tier validation (e.g., premium check).
  - Assert that `can_play` returns `True`.
  - Assert that `player_load_track`, `player_play`, `player_pause`, `player_seek`, `player_set_volume`, and `player_stop` succeed without error.
  - Assert that player event notifications (`Loading`, `Play`, `Pause`, `Ended`, `TimeUpdate`) are emitted.

### 4. Volatility Filtering
Ignore fields that change between runs using `entry.replay_ignore(...)`:
```python
entry.replay_ignore(
    request=BatchHttpRequest(),
    ignore_field=[
        "headers.X-Goog-Visitor-Id",
        "body",
        "headers.user-agent",
    ],
)
entry.replay_ignore(
    request=MainCommand(),
    ignore_fields=["set_secure.data.value.string_value"],
)
```

---

## Credential Sanitization

> [!CAUTION]
> Never commit live OAuth tokens, refresh tokens, or bearer headers to `expectations.json`.

After recording, sanitize all sensitive values:
1. Replace `access_token` and `refresh_token` in HTTP response bodies (`/o/oauth2/token`) with dummy values (e.g., `ya29.dummy_access_token`, `1//dummy_refresh_token`).
2. Replace base64 values in `set_secure` payloads with dummy tokens.
3. Replace `Authorization: Bearer ...` headers in downstream requests with matching dummy tokens.
4. Run `bazel test //extension_name:test` to verify that replay mode passes 100% with sanitized expectations.
