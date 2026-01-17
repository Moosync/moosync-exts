load("@extensions_sdk//:defs.bzl", "rust_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")

rust_extension(
    name = "lastfm",
    package_name = "moosync.lastfm",
    srcs = glob(["src/**/*.rs"]),
    allowed_hosts = [
        "ws.audioscrobbler.com",
    ],
    display_name = "LastFM",
    edition = "2021",
    icon = "assets/icon.svg",
    version = "0.0.3",
    deps = [
        "@lastfm_crates//:lazy_static",
        "@lastfm_crates//:md-5",
        "@lastfm_crates//:regex",
        "@lastfm_crates//:serde",
        "@lastfm_crates//:serde_json",
        "@lastfm_crates//:url",
        "@lastfm_crates//:urlencoding",
    ],
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":lastfm",
)
