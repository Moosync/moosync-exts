"""
Bazel rule to generate ExtensionRegistryManifest manifest.pb from .msxt packages.
"""

def _registry_manifest_impl(ctx):
    out_pb = ctx.outputs.out
    args = ctx.actions.args()
    args.add("--out", out_pb.path)

    inputs = []
    for f in ctx.files.srcs:
        if f.basename.endswith(".msxt"):
            inputs.append(f)
            args.add(f.path)

    key_files = ctx.files.signing_key
    if len(key_files) > 1:
        fail("signing_key must provide at most one key file")
    if len(key_files) == 1:
        args.add("--signing-key", key_files[0].path)
        inputs.append(key_files[0])

    ctx.actions.run(
        executable = ctx.executable._gen_manifest,
        inputs = inputs,
        outputs = [out_pb],
        arguments = [args],
        mnemonic = "GenRegistryManifest",
    )

    return [DefaultInfo(files = depset([out_pb]))]

registry_manifest = rule(
    implementation = _registry_manifest_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True, mandatory = True),
        "out": attr.output(mandatory = True),
        "signing_key": attr.label(
            default = Label("@extensions_sdk//:signing_key"),
            allow_files = True,
        ),
        "_gen_manifest": attr.label(
            default = Label("//tools:gen_manifest"),
            executable = True,
            cfg = "exec",
        ),
    },
)
