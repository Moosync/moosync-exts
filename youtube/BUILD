load("@extensions_sdk//:defs.bzl", "rust_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")

rust_extension(
    name = "youtube",
    package_name = "moosync.youtube",
    srcs = glob(["src/**/*.rs"]),
    allowed_hosts = [
        "*.youtube.com",
        "youtube.com",
        "*.googleapis.com",
    ],
    allowed_paths = {
        "./youtube": "/youtube",
    },
    display_name = "Youtube",
    edition = "2024",
    icon = "assets/icon.svg",
    version = "0.0.2",
    deps = [
        "@youtube_crates//:futures",
        "@youtube_crates//:reqwest",
        "@youtube_crates//:rusty_ytdl",
        "@youtube_crates//:serde",
        "@youtube_crates//:serde_json",
        "@youtube_crates//:url",
    ],
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":youtube",
)
