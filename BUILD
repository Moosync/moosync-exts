load("//tools/dev_registry:dev_registry.bzl", "dev_registry")

package(default_visibility = ["//visibility:public"])

filegroup(
    name = "all",
    srcs = [
        "//discord",
        "//koel",
        "//lastfm",
        "//radio",
        "//soundcloud",
        "//spotify",
        "//youtube",
        "//youtube_dl",
    ],
)

dev_registry(
    name = "dev_registry",
    extensions = [
        "//discord",
        "//koel",
        "//lastfm",
        "//radio",
        "//sample_extensions/go",
        "//sample_extensions/js",
        "//sample_extensions/py",
        "//sample_extensions/rs",
        "//soundcloud",
        "//spotify",
        "//youtube",
        "//youtube_dl",
    ],
)
