---
name: extensions
description: Guidelines and patterns for implementing Moosync extensions across all languages (Rust, Go, Python, JS/TS). Covers scaffolding, Protobuf architecture, provider scopes, incoming ExtensionCommand events, host MainCommand APIs, and testing.
---

# Moosync Extension Authoring Skill

## Overview

Moosync extensions provide streaming, metadata, search, playlist, lyrics, and account integration for music services. Extensions run inside sandboxed WebAssembly (WASM) runtimes (via Wasmtime and Extism) and communicate with Moosync core strictly through host-mediated Protobuf messages.

### Universal Runtime & Sandboxing Constraints
1. **Host-Mediated I/O**: Extensions have no direct OS socket, filesystem, or environment access. All network requests (HTTP), persistent storage (`set_secure`, `get_secure`), and preferences are mediated through host Protobuf messages (`ExtensionCommand` and `MainCommand`).
2. **Single-Threaded Sequential Execution**: WASM runtimes execute single-threaded without background threads or independent background event loops. Detached listeners or unawaited background callbacks will NOT execute once command handling completes. All asynchronous work must be sequentially awaited within the lifecycle of the incoming command invocation.
3. **Strict Memory Management**: Extensions operate within WASM memory limits. Clean up large payloads and avoid memory leaks.

---

## Scaffolding an Extension

Always create new extensions using the Bazel scaffold tool from `@extensions_sdk`:

```bash
bazel run @extensions_sdk//tools:scaffold -- --lang <rust|go|python|js|ts> --name <name> [--dest <path>] [--display-name <name>] [--package-name <pkg>]
```

### Scaffolder Arguments
- `--lang`, `-l`: Extension implementation language (`rust`, `go`, `python`, `py`, `javascript`, `js`, `typescript`, `ts`).
- `--name`, `-n`: Target directory and extension rule name (e.g. `youtube_js`, `spotify`).
- `--dest`, `-d` *(optional)*: Destination directory path (defaults to current working directory / name).
- `--display-name` *(optional)*: Human-readable name displayed in the Moosync UI (defaults to Title Case of name).
- `--package-name` *(optional)*: Unique reverse-DNS identifier (defaults to `moosync.<name>`).

### What Scaffolding Generates
1. **Extension Source Files**: Language-specific entrypoint implementing the Protobuf command handler.
2. **`BUILD.bazel` / `BUILD`**: Defines the language extension rule (`rust_extension`, `go_extension`, `py_extension`, or `js_extension`) and the `moounit_test` target.
3. **`<name>.MODULE.bazel`**: Extension-specific package dependencies (e.g., crates, npm modules, Go packages).
4. **`test.py` & `expectations.json`**: Moounit testing harness and recorded network/host expectations.
5. **Workspace Wiring**: Automatically includes `<name>.MODULE.bazel` in `dependencies.MODULE.bazel` and initializes required toolchains/package locks.

### Build and Test
Always build and test extensions through Bazel — never invoke cargo, npm, or go directly:
```bash
# Build WASM extension target
bazel build //<extension_dir>:<extension_name>

# Run Moounit integration test suite
bazel test //<extension_dir>:test
```

---

## Architecture & IPC Contract

Communication between Moosync core and an extension is strictly governed by Protobuf definitions in `core/types/protos/extensions.proto`.

### Entry Point (Host -> Extension)
Every extension exports a primary entrypoint:
- `handle_extension_command(command: ExtensionCommand) -> ExtensionCommandResponse`

The host passes an `ExtensionCommand` containing a specific event payload. The extension processes the event and returns an `ExtensionCommandResponse` containing the corresponding response payload.

### Host Calls (Extension -> Host)
When the extension needs host services (storing credentials, opening a browser URL, updating account status, modifying local playlists), it issues a `MainCommand` to the host via host-imported functions:
- `send_main_command(command: MainCommand) -> MainCommandResponse`

---

## Provider Scopes (`ExtensionProviderScope`)

When Moosync loads an extension, it sends `GetProviderScopesRequest`. The extension returns a list of supported scopes (`ExtensionProviderScope`) declaring its capabilities:

| Scope Enum | ID | Purpose & Trigger |
| :--- | :--- | :--- |
| `SEARCH` | 0 | Search query resolution for tracks, albums, artists, and playlists (`requested_search_result`). |
| `PLAYLISTS` | 1 | Fetching the user's saved remote playlists (`requested_playlists`). |
| `PLAYLIST_SONGS` | 2 | Fetching songs contained in a remote playlist by ID, with pagination (`requested_playlist_songs`). |
| `ARTIST_SONGS` | 3 | Fetching tracks and releases for an artist ID, with pagination (`requested_artist_songs`). |
| `ALBUM_SONGS` | 4 | Fetching tracks within an album by ID, with pagination (`requested_album_songs`). |
| `RECOMMENDATIONS` | 5 | Fetching personalized or trending song recommendations (`requested_recommendations`). |
| `SCROBBLE` | 6 | Receiving playback progress events to report/scrobble tracks (`scrobble`). |
| `PLAYLIST_FROM_URL` | 7 | Resolving a shareable URL into playlist metadata and track list (`requested_playlist_from_url`). |
| `SONG_FROM_URL` | 8 | Resolving a shareable track URL into Moosync `Song` metadata (`requested_song_from_url`). |
| `PLAYBACK_DETAILS` | 9 | Resolving playable audio stream URL and duration for playback (`playback_details_requested`). |
| `LYRICS` | 10 | Providing synchronized or plain-text lyrics for a track (`requested_lyrics`). |
| `SONG_CONTEXT_MENU` | 11 | Providing custom contextual actions when right-clicking songs (`requested_song_context_menu`). |
| `PLAYLIST_CONTEXT_MENU` | 12 | Providing custom contextual actions when right-clicking playlists (`requested_playlist_context_menu`). |
| `ACCOUNTS` | 13 | Handling user authentication, OAuth flows, and account sync (`perform_account_login`, `oauth_callback`). |
| `DATABASE_SONG_EVENTS` | 14 | Subscribing to library changes when songs are added or removed (`song_added`, `song_removed`). |
| `DATABASE_PLAYLIST_EVENTS` | 15 | Subscribing to playlist changes (`playlist_added`, `playlist_removed`). |
| `PLAYER_UI_EVENTS` | 16 | Subscribing to UI playback controls (`seeked`, `volume_changed`, `player_state_changed`). |
| `PLAYER_DATA_EVENTS` | 17 | Subscribing to track and queue mutations (`song_changed`, `song_queue_changed`). |

---

## Incoming Events (`ExtensionCommand`)

The extension handles the following event requests inside `handle_extension_command`:

### 1. Metadata & Media Resolution
- `requested_search_result(query: string)` -> `RequestedSearchResultResponse`:
  Returns lists of matching `Song`, `Playlist`, `Artist`, and `Album` objects.
- `requested_song_from_url(url: string, refresh: bool)` -> `RequestedSongFromUrlResponse`:
  Resolves a single track URL into a `Song` object.
- `requested_playlist_from_url(url: string, refresh: bool)` -> `RequestedPlaylistFromUrlResponse`:
  Resolves a playlist URL into a `Playlist` object and its `Song` list.
- `requested_playlists(refresh: bool)` -> `RequestedPlaylistsResponse`:
  Returns all remote playlists belonging to the user's logged-in account.
- `requested_playlist_songs(id: string, refresh: bool, page_token?: string)` -> `RequestedPlaylistSongsResponse`:
  Returns paginated songs for a playlist ID and an optional `next_page_token`.
- `requested_artist_songs(artist: Artist, page_token?: string)` -> `RequestedArtistSongsResponse`:
  Returns paginated songs for the given artist.
- `requested_album_songs(album: Album, page_token?: string)` -> `RequestedAlbumSongsResponse`:
  Returns paginated songs for the given album.
- `requested_recommendations(refresh: bool)` -> `RequestedRecommendationsResponse`:
  Returns recommended songs for the home/recommendations view.
- `playback_details_requested(song: Song)` -> `PlaybackDetailsRequestedResponse`:
  Returns the direct audio stream URL and media `Duration`.
- `requested_lyrics(song: Song)` -> `RequestedLyricsResponse`:
  Returns synced or plain-text `Lyrics`.
- `requested_song_from_id(id: string)` -> `RequestedSongFromIdResponse`:
  Returns a `Song` matching the provider's unique song ID.
- `get_remote_url(song: Song)` -> `GetRemoteUrlResponse`:
  Returns the canonical web URL for a song.

### 2. Accounts & Authentication
- `get_provider_scopes()` -> `GetProviderScopesResponse`:
  Reports the `ExtensionProviderScope` list supported by the extension.
- `perform_account_login(account_id: string, login_status: bool)` -> `PerformAccountLoginResponse`:
  Initiates login (e.g. device code flow or OAuth). Returns status text (e.g. user verification code) to be shown in the UI.
- `oauth_callback(callback_uri: string)` -> `OauthCallbackResponse`:
  Handles OAuth redirect URI, completes token exchange, saves tokens via `set_secure`, and calls `set_account`.

### 3. Context Menus & Actions
- `requested_song_context_menu(songs: Song[])` -> `RequestedSongContextMenuResponse`:
  Returns custom context menu action items for selected songs.
- `requested_playlist_context_menu(playlist: Playlist)` -> `RequestedPlaylistContextMenuResponse`:
  Returns custom context menu action items for a playlist.
- `context_menu_action(action_id: string)` -> `ContextMenuActionResponse`:
  Fires when the user clicks a custom context menu item.

### 4. Player & Database Event Subscriptions
- `song_changed(song: Song)`: Notifies when the active playing song changes.
- `player_state_changed(state: PlayerState)`: Notifies on play, pause, or stop.
- `seeked(position: double)`: Notifies when playback position jumps.
- `volume_changed(volume: double)`: Notifies on volume level adjustment.
- `song_queue_changed(queue_state: Struct)`: Notifies on queue changes.
- `song_added(songs: Song[])` / `song_removed(songs: Song[])`: Library song mutations.
- `playlist_added(playlists: Playlist[])` / `playlist_removed(playlists: Playlist[])`: Library playlist mutations.
- `preference_changed(preference: PreferenceArgs)`: Notifies when user edits an extension setting.
- `scrobble(song: Song)`: Triggers when a song has played long enough to qualify for scrobbling.
- `custom_request(request_id: string, payload?: Struct)` -> `CustomRequestResponse`:
  Custom bidirectional communication channel between extension UI and WASM backend.

---

## Outgoing Host APIs (`MainCommand`)

Extensions call the host using `MainCommand`:

### 1. Account & Security Management
- `set_account(ExtensionAccountDetail)`: Registers or updates account state (`id`, `name`, `package_name`, `bg_color`, `icon`, `logged_in`, `username`).
- `register_oauth(OauthDetails)`: Registers OAuth client parameters (client ID, auth URLs) with Moosync core.
- `set_secure(PreferenceData)`: Securely persists encrypted credentials (tokens, refresh keys, secrets).
- `get_secure(PreferenceData)` -> `PreferenceData`: Retrieves encrypted credentials.

### 2. Preferences & Settings
- `register_user_preference(ExtensionPreferenceGroup)`: Registers configurable settings fields displayed in Moosync Preferences.
- `unregister_user_preference(key)`: Removes registered preference fields.
- `get_preference(PreferenceData)` / `set_preference(PreferenceData)`: Reads and writes unencrypted extension preferences.

### 3. System & UI Controls
- `open_external_url(url: string)`: Opens a web URL in the user's default browser (e.g. for OAuth authorization).
- `get_app_version()` -> `string`: Retrieves the running Moosync version.
- `extensions_updated()`: Signals the host that extension configuration or state has updated.

### 4. Library & Playlist Access
- `get_song(GetSongOptions)` -> `Song[]`: Queries songs in the user's Moosync library.
- `get_entity(GetEntityOptions)` -> `EntityResult`: Queries artists, albums, or playlists.
- `add_songs(Song[])`: Inserts songs directly into the user's Moosync library.
- `remove_song(Song)`: Removes a song from the user's library.
- `update_song(Song)`: Updates existing song metadata in the library.
- `add_playlist(Playlist)`: Creates a new playlist in Moosync.
- `add_to_playlist(playlist_id: string, songs: Song[])`: Adds tracks to an existing playlist.

### 5. Player & Playback Inspection
- `get_current_song()` -> `Song`: Returns the currently playing song.
- `get_player_state()` -> `PlayerState`: Returns playing, paused, or stopped state.
- `get_volume()` -> `double`: Returns current volume level.
- `get_time()` -> `double`: Returns current playback position in seconds.
- `get_queue()` -> `Struct`: Returns the active playback queue.

---

## Testing Framework (`moounit`)

Moosync extensions use **Moounit** for hermetic integration testing via Bazel.
- Tests are written in Python (`test.py`) using `moounit_test`.
- Moounit supports **record mode** (`--record`) to capture network traffic and host commands into `expectations.json`, and **replay mode** for hermetic CI runs.
- **Strict Rule**: Never commit unsanitized tokens, client secrets, or bearer headers in `expectations.json`. Always sanitize recorded tokens after recording.
- Detailed testing patterns and best practices are documented in `.agents/skills/moounit/SKILL.md`.

---

## Permissions & Binary Security

All extensions declare their required permissions in their `BUILD` rule (`scopes`, `allowed_hosts`, `allowed_paths`, `binaries`).

### Mandatory Rules
1. **Mandatory Justifications**: Every scope, network host, filesystem path, and binary execution strictly requires an explicit justification string.
2. **Justification is Always the Value**: The permission target is always the key, and the justification string is always the value.
3. **Binary Location in `allowed_paths`**: Any native binary specified in `binaries` must specify its exact execution path (e.g. `path = "{CACHE_DIR}/bin/yt-dlp"`), and the directory containing that binary **must also be explicitly declared in `allowed_paths`**.
4. **Binary Hashes**: Each binary requires at least one cryptographic hash (`sha256`, `sha512`, or `md5`).

### Generating Binary Hashes CLI Tool
Use the `hash_binary` CLI tool provided by `extensions-sdk` to automatically compute hashes and print a ready-to-copy Bazel snippet:

```bash
bazel run @extensions_sdk//tools:hash_binary -- /path/to/binary \
    --name yt-dlp \
    --path "{CACHE_DIR}/bin/yt-dlp" \
    --justification "Extract audio streams from media URLs"
```

Example Bazel rule configuration:
```python
rust_extension(
    name = "my_extension",
    # ...
    allowed_paths = [
        {
            "paths": [
                {"source": "{CACHE_DIR}/bin", "target": "/cache/bin"},
            ],
            "justification": "Store extracted helper binaries",
        },
    ],
    binaries = {
        "yt-dlp": {
            "path": "{CACHE_DIR}/bin/yt-dlp",
            "sha256": "7b819280e5...",
            "sha512": "d80214a9b2...",
            "md5": "9e107d9d37...",
            "justification": "Extract audio streams from media URLs",
        },
    },
)
```

---

## Language-Specific Implementation Skills

For concrete language runtimes, SDK APIs, and packaging details, consult the language skill:
- **JavaScript / TypeScript**: See [extensions-js SKILL](file:///home/ovenoboyo/projects/moosync-exts/.agents/skills/extensions-js/SKILL.md) for QuickJS WASM runtime patterns, `wasm-extension-js` SDK APIs, and rollup bundling with `js_extension`.
- **Rust**: Use `rust_extension` with `prost` protobuf definitions and `extism_pdk`.
- **Go**: Use `go_extension` with rules_go and WebAssembly WASI targets.
