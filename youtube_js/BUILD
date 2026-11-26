load("@extensions_sdk//:defs.bzl", "PROVIDER_SCOPES", "js_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")
load("@npm_youtube_js//:defs.bzl", "npm_link_all_packages")

package(default_visibility = ["//visibility:public"])

npm_link_all_packages(name = "node_modules")

js_extension(
    name = "youtube_js",
    package_name = "app.moosync.youtube.js",
    srcs = [
        "src/cache_keys.ts",
        "src/index.ts",
        "src/model.ts",
    ],
    allowed_hosts = ["*"],
    allowed_paths = {},
    scopes = [
        PROVIDER_SCOPES.ACCOUNTS,
        PROVIDER_SCOPES.ALBUM_SONGS,
        PROVIDER_SCOPES.ARTIST_SONGS,
        PROVIDER_SCOPES.PLAYBACK_DETAILS,
        PROVIDER_SCOPES.PLAYLISTS,
        PROVIDER_SCOPES.PLAYLIST_SONGS,
        PROVIDER_SCOPES.RECOMMENDATIONS,
        PROVIDER_SCOPES.SEARCH,
    ],
    display_name = "Youtube Js",
    node_modules = ":node_modules",
    tsconfig = "tsconfig.json",
    version = "0.0.1",
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    data = ["package.json"],
    debug = True,
    extension = ":youtube_js",
)
