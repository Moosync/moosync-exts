load("@extensions_sdk//:defs.bzl", "PROVIDER_SCOPES", "rust_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")

package(default_visibility = ["//visibility:public"])

rust_extension(
    name = "spotify",
    package_name = "moosync.spotify",
    srcs = glob(["src/**/*.rs"]),
    allowed_hosts = [
        "*",
    ],
    allowed_paths = {
        "{CACHE_DIR}/bin": "/cache/bin",
        "{CACHE_DIR}/soloist_data": "/cache/soloist_data",
    },
    binaries = {
        "soloist": {
            "path": "{CACHE_DIR}/bin/soloist",
            "sha256": "f891edd0acaed42a7893c735819430ef251b7f26b40170e1d89de89c670e7111",
            "sha512": "cfb923b3b759852add4806e69983bab31781a38627430aadaf558b735638b6cb1e23234da12580bb89ec709b551a6419349a4a51a90e2060c10e4cee51adf4d3",
            "md5": "e525166774d5150a203a26cad0208aa4",
            "justification": "Spotify Soloist playback daemon for Spotify Premium streaming",
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
        "@spotify_crates//:chrono",
        "@spotify_crates//:flate2",
        "@spotify_crates//:futures",
        "@spotify_crates//:oauth2",
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
