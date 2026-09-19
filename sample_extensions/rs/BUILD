load("@extensions_sdk//:defs.bzl", "rust_extension")

package(default_visibility = ["//visibility:public"])

rust_extension(
    name = "sample_extension",
    srcs = ["src/lib.rs"],
    deps = [
        "@rs_sample_crates//:serde_json",
    ],
    package_name = "rs.sample",
    display_name = "Sample Extension (Rust)",
    version = "1.0.0",
    allowed_hosts = [
        "api.example.com",
        "*.moosync.app",
    ],
    allowed_paths = {
        "/tmp/moosync": "/tmp/sandbox",
        "/music": "/media/music",
    },
)

alias(
    name = "rs",
    actual = ":sample_extension",
)

