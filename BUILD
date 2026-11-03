load("//tools/dev_registry:dev_registry.bzl", "dev_registry")

package(default_visibility = ["//visibility:public"])

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
        "//youtube",
        "//youtube_dl",
    ],
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
        # "//youtube",
        # "//youtube_dl",
        "//youtube_js"
    ],
)
