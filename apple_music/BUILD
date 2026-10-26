load("@extensions_sdk//:defs.bzl", "PROVIDER_SCOPES", "rust_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")
load("@apple_music_crates//:defs.bzl", "all_crate_deps")

package(default_visibility = ["//visibility:public"])

rust_extension(
    name = "apple_music",
    srcs = glob(["src/**/*.rs"]),
    package_name = "moosync.apple.music",
    display_name = "Apple Music",
    allowed_hosts = [
        "*",
    ],
    allowed_paths = {},
    scopes = [
        PROVIDER_SCOPES.SEARCH,
        PROVIDER_SCOPES.PLAYLIST_SONGS,
        PROVIDER_SCOPES.ALBUM_SONGS,
        PROVIDER_SCOPES.ARTIST_SONGS,
        PROVIDER_SCOPES.RECOMMENDATIONS,
        PROVIDER_SCOPES.PLAYBACK_DETAILS,
        PROVIDER_SCOPES.ACCOUNTS,
    ],
    deps = all_crate_deps(),
    version = "0.0.1",
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":apple_music",
)
