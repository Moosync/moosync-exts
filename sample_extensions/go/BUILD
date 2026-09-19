load("@extensions_sdk//:defs.bzl", "go_extension")

package(default_visibility = ["//visibility:public"])

go_extension(
    name = "sample_extension",
    srcs = ["main.go"],
    package_name = "go.sample",
    display_name = "Sample Extension (Go)",
    version = "1.0.0",
)

alias(
    name = "go",
    actual = ":sample_extension",
)
