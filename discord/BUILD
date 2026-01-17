load("@extensions_sdk//:defs.bzl", "rust_extension")
load("@extensions_sdk//moounit:defs.bzl", "moounit_test")

rust_extension(
    name = "discord",
    package_name = "moosync.discord.rpc",
    srcs = glob(["src/**/*.rs"]),
    allowed_hosts = [],
    allowed_paths = {
        "{XDG_RUNTIME_DIR}": "/",
        "/tmp": "/",
        "{TEMP_DIR}": "/",
        "{TMPDIR}": "/",
        "{TMP}": "/",
        "{TEMP}": "/",
        "{XDG_RUNTIME_DIR}/app/com.discordapp.Discord": "/",
        "\\\\.\\pipe\\": "/",
        "{XDG_RUNTIME_DIR}/.flatpak/dev.vencord.Vesktop/xdg-run": "/",
    },
    display_name = "Discord RPC",
    edition = "2024",
    icon = "assets/icon.svg",
    version = "0.0.2",
    deps = [
        "@discord_crates//:bytes",
        "@discord_crates//:getrandom",
        "@discord_crates//:serde",
        "@discord_crates//:serde_json",
    ],
)

moounit_test(
    name = "test",
    srcs = ["test.py"],
    debug = True,
    extension = ":discord",
)
