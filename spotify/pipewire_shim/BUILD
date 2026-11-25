load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_shared_library", "rust_test")

package(default_visibility = ["//visibility:public"])

rust_library(
    name = "pw_aaudio_shm",
    srcs = ["src/shm.rs"],
    edition = "2024",
)

SHIM_SRCS = [
    "src/android_init.rs",
    "src/dns.rs",
    "src/event_loop.rs",
    "src/ffi.rs",
    "src/lib.rs",
    "src/shm.rs",
    "src/spa.rs",
    "src/stream.rs",
]

rust_library(
    name = "pipewire_shim_lib",
    srcs = SHIM_SRCS,
    edition = "2024",
    deps = [
        "@spotify_crates//:simple-dns",
    ],
)

rust_shared_library(
    name = "pipewire_shim_so",
    crate_name = "pipewire_shim",
    srcs = SHIM_SRCS,
    edition = "2024",
    platform = "@platforms//host",
    rustc_flags = [
        "-Clink-arg=-static-libgcc",
        "-Clink-arg=-Wl,-z,max-page-size=16384",
    ],
    deps = [
        "@spotify_crates//:simple-dns",
    ],
)

genrule(
    name = "libpipewire_so",
    srcs = [":pipewire_shim_so"],
    outs = ["libpipewire-0.3.so.0"],
    cmd = "cp $< $@",
)

rust_test(
    name = "pw_shim_unit_test",
    crate = ":pipewire_shim_lib",
    edition = "2024",
)

rust_test(
    name = "pw_shim_test",
    srcs = ["tests/pw_shim_test.rs"],
    edition = "2024",
    deps = [":pipewire_shim_lib"],
)

rust_binary(
    name = "pw_sine_poc",
    srcs = ["tests/pw_sine_poc.rs"],
    edition = "2024",
    rustc_flags = [
        "-Clink-arg=-static-libgcc",
        "-Clink-arg=-Wl,-z,max-page-size=16384",
    ],
    deps = [":pipewire_shim_lib"],
)
