load("@extensions_sdk//:defs.bzl", "PROVIDER_SCOPES", "rust_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")
load("@lyrics_crates//:defs.bzl", "all_crate_deps")

package(default_visibility = ["//visibility:public"])

rust_extension(
    name = "lyrics",
    package_name = "app.moosync.lyrics",
    srcs = glob(["src/**/*.rs"]),
    allowed_hosts = [
        "*.lrclib.net",
        "lrclib.net",
        "api.genius.com",
        "genius.com",
    ],
    display_name = "Lyrics (LRCLIB & Genius)",
    edition = "2024",
    scopes = [
        PROVIDER_SCOPES.LYRICS,
    ],
    version = "0.0.1",
    deps = all_crate_deps(),
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":lyrics",
)
