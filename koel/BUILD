load("@extensions_sdk//:defs.bzl", "rust_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")

rust_extension(
    name = "koel",
    package_name = "moosync.koel",
    srcs = glob(["src/**/*.rs"]),
    allowed_hosts = [
        "*",
    ],
    display_name = "Koel",
    edition = "2024",
    icon = "assets/icon.png",
    version = "0.0.2",
    deps = [
        "@koel_crates//:futures",
        "@koel_crates//:reqwest",
        "@koel_crates//:serde",
        "@koel_crates//:serde_json",
        "@koel_crates//:urlencoding",
    ],
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":koel",
)
