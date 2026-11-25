load("@rules_cc//cc:defs.bzl", "cc_import")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_test")

package(default_visibility = ["//visibility:public"])

cc_import(
    name = "android_libdl",
    shared_library = select({
        "@platforms//cpu:aarch64": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/aarch64-linux-android/31/libdl.so",
        "@platforms//cpu:x86_64": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/x86_64-linux-android/31/libdl.so",
        "//conditions:default": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/x86_64-linux-android/31/libdl.so",
    }),
)

cc_import(
    name = "android_libc",
    shared_library = select({
        "@platforms//cpu:aarch64": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/aarch64-linux-android/31/libc.so",
        "@platforms//cpu:x86_64": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/x86_64-linux-android/31/libc.so",
        "//conditions:default": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/x86_64-linux-android/31/libc.so",
    }),
)

cc_import(
    name = "android_libm",
    shared_library = select({
        "@platforms//cpu:aarch64": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/aarch64-linux-android/31/libm.so",
        "@platforms//cpu:x86_64": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/x86_64-linux-android/31/libm.so",
        "//conditions:default": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/x86_64-linux-android/31/libm.so",
    }),
)

cc_import(
    name = "android_libaaudio",
    shared_library = select({
        "@platforms//cpu:aarch64": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/aarch64-linux-android/31/libaaudio.so",
        "@platforms//cpu:x86_64": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/x86_64-linux-android/31/libaaudio.so",
        "//conditions:default": "@androidndk//toolchains/llvm/prebuilt/linux-x86_64/sysroot:usr/lib/x86_64-linux-android/31/libaaudio.so",
    }),
)

rust_library(
    name = "aaudio_ring",
    srcs = ["src/ring.rs"],
    edition = "2024",
    deps = [
        "//spotify/pipewire_shim:pw_aaudio_shm",
    ],
)

rust_test(
    name = "aaudio_sink_test",
    crate = ":aaudio_ring",
    edition = "2024",
)

rust_binary(
    name = "aaudio_sink",
    srcs = [
        "src/main.rs",
        "src/ring.rs",
    ],
    edition = "2024",
    link_deps = [
        ":android_libaaudio",
        ":android_libc",
        ":android_libdl",
        ":android_libm",
    ],
    deps = [
        "//spotify/pipewire_shim:pw_aaudio_shm",
    ],
)

genrule(
    name = "libaaudio_sink_so",
    srcs = [":aaudio_sink"],
    outs = ["libaaudio_sink.so"],
    cmd = "cp $< $@",
)

rust_binary(
    name = "aaudio_sink_x86_64",
    srcs = [
        "src/main.rs",
        "src/ring.rs",
    ],
    edition = "2024",
    platform = "@moosync//:x86_64",
    link_deps = [
        ":android_libaaudio",
        ":android_libc",
        ":android_libdl",
        ":android_libm",
    ],
    deps = [
        "//spotify/pipewire_shim:pw_aaudio_shm",
    ],
)
