load("//tools/dev_registry:dev_registry.bzl", "dev_registry")
load("//tools:registry_manifest.bzl", "registry_manifest")

package(default_visibility = ["//visibility:public"])

exports_files(glob(["*.asc"], allow_empty = True))

filegroup(
    name = "all",
    srcs = [
        "//cover_art",
        "//discord",
        "//koel",
        "//lastfm",
        "//lyrics",
        "//radio",
        "//soundcloud",
        "//spotify",
        "//youtube_js",
    ],
)

registry_manifest(
    name = "manifest",
    srcs = [":all"],
    out = "manifest.pb",
)

dev_registry(
    name = "dev_registry",
    extensions = [
        "//cover_art",
        "//discord",
        "//koel",
        "//lastfm",
        "//lyrics",
        "//radio",
        "//soundcloud",
        "//spotify",
        "//youtube_js",
    ],
)
