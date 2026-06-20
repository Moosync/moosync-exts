load("@extensions_sdk//:defs.bzl", "py_extension")

package(default_visibility = ["//visibility:public"])

py_extension(
    name = "sample_extension",
    srcs = ["main.py"],
    main = "@extensions_sdk//wasm-extension-py:moosync_edk/__init__.py",
    package_name = "py.sample",
    display_name = "Sample Extension (Python)",
    version = "1.0.0",
)
