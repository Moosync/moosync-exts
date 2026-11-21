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
        "//youtube_js",
    ],
)

genrule(
    name = "manifest",
    srcs = [":all"],
    outs = ["manifest.pb"],
    cmd = "$(location //tools:gen_manifest) --out $@ $(SRCS)",
    tools = ["//tools:gen_manifest"],
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
