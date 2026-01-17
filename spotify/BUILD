load("@extensions_sdk//:defs.bzl", "rust_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")

rust_extension(
    name = "spotify",
    package_name = "moosync.spotify",
    srcs = glob(["src/**/*.rs"]),
    allowed_hosts = [
        "*",
    ],
    display_name = "Spotify",
    edition = "2024",
    icon = "assets/icon.png",
    version = "0.0.1",
    deps = [
        "@spotify_crates//:chrono",
        "@spotify_crates//:futures",
        "@spotify_crates//:oauth2",
        "@spotify_crates//:regex",
        "@spotify_crates//:reqwest",
        "@spotify_crates//:rspotify",
        "@spotify_crates//:serde",
        "@spotify_crates//:serde_json",
        "@spotify_crates//:url",
    ],
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":spotify",
)
