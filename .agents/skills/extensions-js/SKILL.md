---
name: extensions-js
description: Authoring Moosync extensions in JavaScript/TypeScript for the QuickJS WASM runtime. Covers wasm-extension-js SDK APIs, event registration (api.on), single-threaded execution constraints, bundling with Bazel (js_extension), and Moounit testing.
---

# JavaScript & TypeScript Moosync Extension Authoring Skill

## Overview

Moosync JavaScript and TypeScript extensions run inside a sandboxed WebAssembly (WASM) runtime powered by QuickJS. They use the `wasm-extension-js` SDK package to interact with Moosync core via Protobuf messages.

---

## QuickJS WASM Runtime Constraints

### 1. Single-Threaded Sequential Execution
- QuickJS executes in a WASM sandbox without OS background threads or preemptive event loops.
- **WASM execution only progresses while a host command invocation is active.**
- **No Background Callbacks**: Event emitter callbacks registered on external libraries (e.g. `client.on("event", ...)`) will **NOT** fire once the current command execution returns.
- **Mandatory Sequential Await**: All asynchronous operations must be explicitly awaited inside the handler before returning the response.

### 2. Sandbox & Environment Limitations
- **No Direct Network or File I/O**: Direct Node.js `fs`, `net`, or native C++ modules cannot run. Network requests (`fetch`) and storage are mediated through the Moosync host.
- **Polyfills**: QuickJS supports modern ES2020+ JavaScript syntax, but certain global APIs expected by web/Node packages may be absent:
  - `Intl`: If third-party libraries require `Intl.DateTimeFormat`, polyfill minimal stub:
    ```typescript
    if (typeof (globalThis as unknown as { Intl: unknown }).Intl === "undefined") {
      (globalThis as unknown as { Intl: unknown }).Intl = {
        DateTimeFormat: () => ({
          resolvedOptions: () => ({ timeZone: "UTC" }),
        }),
      };
    }
    ```
  - `Buffer` / `atob` / `btoa`: Use standard `Uint8Array`, `TextEncoder`, and `TextDecoder`.

---

## Project Structure & Bazel Configuration

A typical JS/TS extension consists of:
```
<extension_name>/
├── BUILD
├── <extension_name>.MODULE.bazel
├── package.json
├── pnpm-lock.yaml
├── tsconfig.json
├── src/
│   ├── index.ts      # Main entry point & event handlers
│   └── model.ts      # Protobuf data mappers & helpers
├── test.py           # Moounit integration test suite
└── expectations.json # Recorded network & host mocks
```

### 1. `BUILD`
Defines the `js_extension` and `moounit_test` targets:
```python
load("@extensions_sdk//:defs.bzl", "js_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")
load("@npm_<name>//:defs.bzl", "npm_link_all_packages")

package(default_visibility = ["//visibility:public"])

npm_link_all_packages(name = "node_modules")

js_extension(
    name = "<name>",
    package_name = "moosync.<name>",
    srcs = glob(["src/**/*.ts"]),
    allowed_hosts = ["*"],  # Whitelist required API domains or ["*"]
    allowed_paths = {},
    display_name = "<Display Name>",
    node_modules = ":node_modules",
    package_json = "package.json",
    tsconfig = "tsconfig.json",
    version = "0.0.1",
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":<name>",
)
```

### 2. `<name>.MODULE.bazel`
Declares pnpm lockfile translation for Aspect rules_js:
```python
npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")

npm.npm_translate_lock(
    name = "npm_<name>",
    pnpm_lock = "//<name>:pnpm-lock.yaml" if "//<name>" else "//:pnpm-lock.yaml",
    update_pnpm_lock = True,
    data = ["//<name>:package.json" if "//<name>" else "//:package.json"],
)
use_repo(npm, "npm_<name>")
```

### 3. `package.json`
```json
{
  "name": "<name>",
  "version": "0.0.1",
  "type": "module",
  "dependencies": {
    "@types/node": "^25.0.3"
  },
  "pnpm": {
    "onlyBuiltDependencies": []
  }
}
```

---

## Extension Entry Point & Lifecycle

Every JS extension must export `handle_extension_command` from `wasm-extension-js` and an `entry()` function:

```typescript
import {
  ExtensionCommand,
  ExtensionCommandResponse,
  getApi,
  handle_extension_command,
} from "wasm-extension-js";

export { handle_extension_command };

const api = getApi();

class MyExtension {
  public async init() {
    this.register_api();
  }

  private register_api() {
    // Register event listeners
  }
}

let extension: MyExtension;

export function entry(): number {
  extension = new MyExtension();
  extension.init();
  console.log("Extension initialized successfully");
  return 0;
}
```

---

## Registering Event Handlers (`api.on`)

The `wasm-extension-js` SDK routes incoming `ExtensionCommand` events via `api.on(eventName, handler)`. All handlers can be synchronous or return a `Promise`.

### 1. Provider Capabilities
```typescript
import { ExtensionProviderScope, GetProviderScopesResponse } from "wasm-extension-js";

api.on("getProviderScopes", (_req) => {
  return new GetProviderScopesResponse({
    scopes: [
      ExtensionProviderScope.SEARCH,
      ExtensionProviderScope.SONG_FROM_URL,
      ExtensionProviderScope.PLAYLIST_FROM_URL,
      ExtensionProviderScope.PLAYLISTS,
      ExtensionProviderScope.PLAYLIST_SONGS,
      ExtensionProviderScope.ARTIST_SONGS,
      ExtensionProviderScope.ALBUM_SONGS,
      ExtensionProviderScope.RECOMMENDATIONS,
      ExtensionProviderScope.ACCOUNTS,
    ],
  });
});
```

### 2. Search & Queries
```typescript
import {
  RequestedSearchResultResponse,
  RequestedSongFromUrlResponse,
  RequestedPlaylistFromUrlResponse,
  RequestedPlaylistsResponse,
  RequestedPlaylistSongsResponse,
  RequestedArtistSongsResponse,
  RequestedAlbumSongsResponse,
  RequestedRecommendationsResponse,
  RequestedLyricsResponse,
  PlaybackDetailsRequestedResponse,
  Song,
  Playlist,
  Artist,
  Album,
} from "wasm-extension-js";

// Search
api.on("requestedSearchResult", async ({ query }) => {
  const { songs, playlists, artists, albums } = await fetchSearchResults(query);
  return new RequestedSearchResultResponse({ songs, playlists, artists, albums });
});

// Single Track URL
api.on("requestedSongFromUrl", async ({ url }) => {
  const song = await parseSongUrl(url);
  return new RequestedSongFromUrlResponse({ song });
});

// Playlist URL
api.on("requestedPlaylistFromUrl", async ({ url }) => {
  const { playlist, songs } = await parsePlaylistUrl(url);
  return new RequestedPlaylistFromUrlResponse({ playlist, songs });
});

// User's Remote Playlists
api.on("requestedPlaylists", async ({ refresh }) => {
  const playlists = await fetchUserPlaylists(refresh);
  return new RequestedPlaylistsResponse({ playlists });
});

// Playlist Tracks (Paginated)
api.on("requestedPlaylistSongs", async ({ id, pageToken, refresh }) => {
  const { songs, nextPageToken } = await fetchPlaylistTracks(id, pageToken, refresh);
  return new RequestedPlaylistSongsResponse({ songs, nextPageToken });
});

// Playback Details (Audio Stream URL + Duration)
api.on("playbackDetailsRequested", async ({ song }) => {
  const { streamUrl, durationSeconds } = await resolveStream(song);
  return new PlaybackDetailsRequestedResponse({
    url: streamUrl,
    duration: { seconds: BigInt(Math.floor(durationSeconds)), nanos: 0 },
  });
});

// Lyrics
api.on("requestedLyrics", async ({ song }) => {
  const lyrics = await fetchLyrics(song);
  return new RequestedLyricsResponse({ lyrics });
});
```

### 3. Account Login & OAuth
```typescript
import { PerformAccountLoginResponse, OauthCallbackResponse } from "wasm-extension-js";

// Triggered when user clicks "Login" in Moosync Settings
api.on("performAccountLogin", async ({ loginStatus, accountId }) => {
  if (loginStatus) {
    const { userCode, verificationUrl } = await requestDeviceCode();
    // Direct user to authorization page
    api.openExternalUrl(verificationUrl);
    // Return user code to show in Moosync UI
    return new PerformAccountLoginResponse({ status: userCode });
  } else {
    // Logout flow
    await clearSavedTokens();
    await syncLoggedOutAccount();
    return new PerformAccountLoginResponse({ status: "Logged out" });
  }
});

// Triggered when OAuth redirects to moosync://oauth-callback
api.on("oauthCallback", async ({ callbackUri }) => {
  await exchangeCodeAndSave(callbackUri);
  await syncLoggedInAccount();
  return new OauthCallbackResponse();
});
```

### 4. Player & Library Event Subscriptions
```typescript
api.on("songChanged", ({ song }) => {
  console.log("Song changed to:", song?.title);
});

api.on("playerStateChanged", ({ state }) => {
  console.log("Player state changed:", state);
});

api.on("preferenceChanged", ({ preference }) => {
  console.log("Setting changed:", preference?.key, preference?.value);
});
```

---

## Invoking Host APIs (`api`)

The `api` object returned by `getApi()` provides synchronous and asynchronous methods to call Moosync:

### 1. Syncing Account Status (`api.setAccount`)
Call this on startup, after login, and after logout:
```typescript
import { ExtensionAccountDetail } from "wasm-extension-js";

api.setAccount(
  new ExtensionAccountDetail({
    id: "my_account_id",
    name: "My Service",
    packageName: "moosync.my_extension",
    bgColor: "#1DB954",
    icon: "",
    loggedIn: true,
    username: "User123",
  })
);
```

### 2. Secure Token Storage (`api.setSecure` & `api.getSecure`)
Persist sensitive credentials like OAuth tokens encrypted on the host:
```typescript
import { PreferenceData } from "wasm-extension-js";

// Save encrypted token
function saveToken(token: string) {
  const b64 = btoa(token);
  api.setSecure(
    new PreferenceData({
      key: "oauth_access_token",
      value: { kind: { case: "stringValue", value: b64 } },
    })
  );
}

// Retrieve encrypted token
function loadToken(): string | undefined {
  const data = api.getSecure(new PreferenceData({ key: "oauth_access_token" }));
  const val = data?.value;
  if (val?.kind.case === "stringValue" && val.kind.value) {
    return atob(val.kind.value);
  }
  return undefined;
}
```

### 3. Opening External URLs (`api.openExternalUrl`)
```typescript
api.openExternalUrl("https://service.com/oauth/authorize?...");
```

### 4. User Preferences (`api.registerUserPreferences`, `api.getPreference`)
Register configurable options shown in Moosync Settings:
```typescript
import { ExtensionPreferenceGroup, PreferenceType } from "wasm-extension-js";

api.registerUserPreferences(
  new ExtensionPreferenceGroup({
    key: "general",
    title: "General Settings",
    description: "Configure extension options",
    preferences: [
      {
        key: "audio_quality",
        title: "Audio Quality",
        description: "Select streaming bitrate",
        type: PreferenceType.DROPDOWN,
        defaultValue: { kind: { case: "stringValue", value: "high" } },
        options: [
          { key: "low", title: "Low (96kbps)" },
          { key: "high", title: "High (320kbps)" },
        ],
      },
    ],
  })
);
```

---

## TypeScript Guidelines & Best Practices

1. **Zero `any` and Zero `as`**:
   - Strictly type all models and functions.
   - Do not cast using `as unknown as Type`. Use Protobuf constructors (`new Song({ ... })`) and narrowing/type guards (`if (item instanceof ...)` or `if (typeof x === "string")`).
2. **Transforming External API Data into Protobuf**:
   Always sanitize IDs and strings before creating Protobuf objects:
   ```typescript
   export function parseSong(item: ExternalTrack): Song {
     return new Song({
       id: String(item.id),
       title: item.name ?? "Unknown Title",
       artists: item.artists?.map((a) => a.name) ?? [],
       album: item.album?.name ?? "",
       duration: item.duration_ms ? item.duration_ms / 1000 : 0,
       playbackUrl: item.id,
     });
   }
   ```
3. **ArrayBuffer Cloning**:
   If passing buffers across WASM memory boundary or storing binary cache:
   ```typescript
   function cloneToArrayBuffer(buf: ArrayBufferLike): ArrayBuffer {
     const target = new ArrayBuffer(buf.byteLength);
     new Uint8Array(target).set(new Uint8Array(buf));
     return target;
   }
   ```

---

## Testing with Moounit

Every JS extension includes a `test.py` harness running under `moounit_test`:

```bash
# Run unit/integration tests
bazel test //<extension_name>:test

# Record network requests and host commands
bazel run //<extension_name>:test -- --record
```

### Structure of `test.py`
```python
import unittest
from moounit.extension_test_handler import ExtensionTestHandler
from moosync_types.extensions_pb2 import (
    ExtensionCommand,
    ExtensionProviderScope,
    RequestedSearchResultRequest,
)

class MyExtensionTest(unittest.TestCase):
    def setUp(self):
        self.handler = ExtensionTestHandler()
        self.handler.init()

    def test_provider_scopes(self):
        cmd = ExtensionCommand()
        cmd.get_provider_scopes.SetInParent()
        resp = self.handler.handle_command(cmd)
        self.assertIn(ExtensionProviderScope.SEARCH, resp.get_provider_scopes.scopes)

    def test_search(self):
        cmd = ExtensionCommand()
        cmd.requested_search_result.query = "test query"
        resp = self.handler.handle_command(cmd)
        self.assertGreater(len(resp.requested_search_result.songs), 0)
        self.assertEqual(resp.requested_search_result.songs[0].id, "expected_id")
```

### Sanitizing `expectations.json`
After recording tests with `--record`, always sanitize `expectations.json` before committing:
- Remove live OAuth access tokens, refresh tokens, and cookies.
- Replace client secrets or authorization headers with dummy strings.
