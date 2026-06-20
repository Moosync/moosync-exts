load("@extensions_sdk//:defs.bzl", "go_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")

package(default_visibility = ["//visibility:public"])

go_extension(
    name = "soundcloud",
    package_name = "moosync.soundcloud",
    srcs = [
        "main.go",
        "utils.go",
    ],
    allowed_hosts = [
        "*",
    ],
    display_name = "Soundcloud",
    icon = "assets/icon.svg",
    version = "2.1.2",
    deps = [
        "@com_github_antchfx_htmlquery//:htmlquery",
        "@com_github_ovenoboyo_scdl_v2//pkg/soundcloud",
        "@com_github_zackradisic_soundcloud_api//:soundcloud-api",
        "@org_golang_google_protobuf//types/known/durationpb",
    ],
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":soundcloud",
)
