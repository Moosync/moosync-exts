load("@extensions_sdk//:defs.bzl", "js_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")
load("@npm_youtube_js//:defs.bzl", "npm_link_all_packages")

package(default_visibility = ["//visibility:public"])

npm_link_all_packages(name = "node_modules")

js_extension(
    name = "youtube_js",
    package_name = "moosync.youtube.js",
    srcs = [
        "src/index.ts",
        "src/model.ts",
    ],
    allowed_hosts = ["*"],
    allowed_paths = {},
    display_name = "Youtube Js",
    node_modules = ":node_modules",
    package_json = "package.json",
    tsconfig = "tsconfig.json",
    version = "0.0.1",
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":youtube_js",
)
