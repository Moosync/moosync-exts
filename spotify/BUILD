load("@extensions_sdk//:defs.bzl", "PROVIDER_SCOPES", "rust_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")
load("@rules_python//python:defs.bzl", "py_binary")
load("@rules_rust//rust:defs.bzl", "rust_test")

package(default_visibility = ["//visibility:public"])

rust_extension(
    name = "spotify",
    package_name = "app.moosync.spotify",
    srcs = glob(["src/**/*.rs"]),
    allowed_hosts = [
        "*",
    ],
    allowed_paths = {
        "{CACHE_DIR}/bin": "/cache/bin",
        "{CACHE_DIR}/glibc": "/cache/glibc",
        "{CACHE_DIR}/soloist_data": "/cache/soloist_data",
    },
    binaries = {
        "soloist": {
            "path": "{CACHE_DIR}/bin/soloist",
            "sha256": "34eb42aae73d9db8be0be91183b3f1fa9625f3dbac538ceede9daffbbe6340c7",
            "sha512": "dd5a401773b58983bc2f3144bee6ed05836c4c4a0e3a3366544ca8f7e6f85c22e89f0eb32af4989709464a62757eaab6f9d13f863655abe4209d73da2c22f100",
            "md5": "2609beb80d836a494432e3dcc964c9a6",
            "justification": "Spotify Soloist playback daemon for Spotify Premium streaming",
        },
        "soloist_android_x86_64": {
            "path": "{CACHE_DIR}/bin/soloist_android_x86_64",
            "sha256": "1448228a0f98efeff5843e26df30222a994bca8143d5ab387a9581f0fc90291a",
            "sha512": "a1d58678cb9ef441c51789e782f92127a48e45b4212e401410d91576e34ac3dfd50275a55c4b91352fe8ca62a3dd6e6140c2c2a51df7d33eaf5d5c611e715559",
            "md5": "5d820fb0be3e4c59459969dc6c615801",
            "justification": "16KB-aligned Spotify Soloist playback daemon for Android x86_64",
        },
        "aaudio_sink": {
            "path": "{CACHE_DIR}/bin/aaudio_sink",
            "sha256": "fcaf1ebc6c7f08c38d4def8283297d84cb1cdb37391ef724cb166abedc1000aa",
            "sha512": "733944cc991bc0062e46d30643fef50d2db54e082cbdbb9a28713dae12c98daaa007e20e07aecbc85edae6917d0fac836de372a2154303e799a6f11e404b212b",
            "md5": "14ba828e9547dda5fed1cae1bc707b8b",
            "justification": "AAudio shared-memory audio sink for Spotify Soloist on Android",
        },
    },
    scopes = [
        PROVIDER_SCOPES.ACCOUNTS,
        PROVIDER_SCOPES.SEARCH,
        PROVIDER_SCOPES.PLAYLIST_SONGS,
        PROVIDER_SCOPES.PLAYLISTS,
        PROVIDER_SCOPES.ARTIST_SONGS,
        PROVIDER_SCOPES.ALBUM_SONGS,
        PROVIDER_SCOPES.RECOMMENDATIONS,
        PROVIDER_SCOPES.PLAYER_PROVIDER,
        PROVIDER_SCOPES.PLAYER_DATA_EVENTS,
        PROVIDER_SCOPES.PLAYER_UI_EVENTS,
    ],
    display_name = "Spotify",
    edition = "2024",
    icon = "assets/icon.png",
    version = "0.0.1",
    deps = [
        "//spotify/glibc:glibc_bundle",
        "@spotify_crates//:chrono",
        "@spotify_crates//:flate2",
        "@spotify_crates//:futures",
        "@spotify_crates//:oauth2",
        "@spotify_crates//:object",
        "@spotify_crates//:regex",
        "@spotify_crates//:reqwest",
        "@spotify_crates//:rspotify",
        "@spotify_crates//:serde",
        "@spotify_crates//:serde_json",
        "@spotify_crates//:tar",
        "@spotify_crates//:url",
        "@spotify_crates//:zip",
    ],
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    data = glob([".env"], allow_empty = True),
    debug = True,
    extension = ":spotify",
)

py_binary(
    name = "update_hashes",
    srcs = ["tools/update_hashes.py"],
    args = ["--aaudio-sink=$(location //spotify/aaudio_sink:aaudio_sink_x86_64)"],
    data = ["//spotify/aaudio_sink:aaudio_sink_x86_64"],
    deps = ["@extensions_sdk//tools:elf_align_lib"],
)

rust_test(
    name = "elf_align_test",
    srcs = ["src/soloist/elf_align.rs"],
    edition = "2024",
    deps = [
        "@spotify_crates//:object",
    ],
)
