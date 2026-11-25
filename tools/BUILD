load("@rules_python//python:defs.bzl", "py_binary")

package(default_visibility = ["//visibility:public"])

py_binary(
    name = "gen_manifest",
    srcs = ["gen_manifest.py"],
    main = "gen_manifest.py",
    deps = [
        "@extensions_sdk//protos:extensions_py_proto",
        "@extensions_sdk//protos:moosync_python_root",
    ],
)

py_binary(
    name = "check_git_override",
    srcs = ["check_git_override.py"],
    main = "check_git_override.py",
)
