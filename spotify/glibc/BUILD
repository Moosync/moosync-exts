load("@rules_python//python:defs.bzl", "py_binary", "py_library")
load("@rules_rust//rust:defs.bzl", "rust_library")

package(default_visibility = ["//visibility:public"])

py_library(
    name = "pack_glibc_lib",
    srcs = ["pack_glibc.py"],
    deps = ["@extensions_sdk//tools:elf_align_lib"],
)

py_binary(
    name = "pack_glibc",
    srcs = ["pack_glibc.py"],
    deps = ["@extensions_sdk//tools:elf_align_lib"],
)

genrule(
    name = "glibc_x86_64_tar_gz",
    srcs = [
        "@termux_glibc_x86_64//file",
        "@termux_gcc_libs_x86_64//file",
        "//spotify/pipewire_shim:libpipewire_so",
    ],
    outs = ["glibc_x86_64.tar.gz"],
    cmd = "$(location :pack_glibc) $@ $(location //spotify/pipewire_shim:libpipewire_so) $(location @termux_glibc_x86_64//file) $(location @termux_gcc_libs_x86_64//file)",
    tools = [":pack_glibc"],
)

rust_library(
    name = "glibc_bundle",
    srcs = ["src/lib.rs"],
    compile_data = [
        ":glibc_x86_64_tar_gz",
        "//spotify/aaudio_sink:aaudio_sink_x86_64",
    ],
    edition = "2024",
    rustc_env = {
        "GLIBC_X86_64_TAR_GZ": "$(location :glibc_x86_64_tar_gz)",
        "AAUDIO_SINK_X86_64": "$(location //spotify/aaudio_sink:aaudio_sink_x86_64)",
    },
)
