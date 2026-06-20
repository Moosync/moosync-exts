load("@extensions_sdk//:defs.bzl", "py_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")
load("@pypi_youtube_dl//:requirements.bzl", "requirement")
load("@rules_python//python:pip.bzl", "compile_pip_requirements")

package(default_visibility = ["//visibility:public"])

compile_pip_requirements(
    name = "requirements",
    src = "requirements.txt",
    generate_hashes = False,
    requirements_txt = "requirements_lock.txt",
)

py_extension(
    name = "youtube_dl",
    package_name = "moosync.youtube_dl",
    srcs = [
        "extension.py",
        "youtube_dl.py",
    ],
    allowed_hosts = [
        "*.youtube.com",
        "youtube.com",
        "*.googlevideo.com",
        "cipher.moosync.app",
    ],
    allowed_paths = {
        "{CACHE_DIR}/youtube_dl/test": "/downloads",
    },
    display_name = "Youtube (No API)",
    icon = "assets/icon.svg",
    main = "extension.py",
    version = "0.0.1",
    deps = [
        requirement("yt-dlp"),
        requirement("yt-dlp-remote-cipher"),
    ],
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":youtube_dl",
)
