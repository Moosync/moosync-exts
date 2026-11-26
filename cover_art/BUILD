load("@extensions_sdk//:defs.bzl", "PROVIDER_SCOPES", "rust_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")
load("@cover_art_crates//:defs.bzl", "all_crate_deps")

package(default_visibility = ["//visibility:public"])

rust_extension(
    name = "cover_art",
    package_name = "app.moosync.cover_art",
    srcs = glob(["src/**/*.rs"]),
    allowed_hosts = [
        "api.deezer.com",
        "*.dzcdn.net",
        "itunes.apple.com",
        "*.mzstatic.com",
        "musicbrainz.org",
        "coverartarchive.org",
        "archive.org",
    ],
    display_name = "Cover Art (Deezer, iTunes, MusicBrainz)",
    edition = "2024",
    scopes = [
        PROVIDER_SCOPES.COVER_ART,
    ],
    version = "0.0.1",
    deps = all_crate_deps(),
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":cover_art",
)
