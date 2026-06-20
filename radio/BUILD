load("@extensions_sdk//:defs.bzl", "rust_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")

package(default_visibility = ["//visibility:public"])

rust_extension(
    name = "radio",
    package_name = "moosync.radio",
    srcs = glob(["src/**/*.rs"]),
    allowed_hosts = [
        "*.api.radio-browser.info",
        "api.radio-browser.info",
    ],
    display_name = "Radio",
    edition = "2024",
    icon = "assets/icon.svg",
    version = "0.0.2",
    deps = [
        "@radio_crates//:futures",
        "@radio_crates//:radiobrowser",
        "@radio_crates//:regex",
    ],
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":radio",
)
