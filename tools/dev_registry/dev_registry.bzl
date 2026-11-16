"""
Bazel rule for starting a local development registry for Moosync extensions.
"""

def _dev_registry_impl(ctx):
    all_files = []
    for ext in ctx.attr.extensions:
        all_files.extend(ext[DefaultInfo].files.to_list())
        if ext[DefaultInfo].default_runfiles:
            all_files.extend(ext[DefaultInfo].default_runfiles.files.to_list())

    proto_files = []
    python_paths = ["${RUNFILES_DIR}/_main"]
    for dep in ctx.attr._proto_deps:
        proto_files.extend(dep[DefaultInfo].files.to_list())
        if dep[DefaultInfo].default_runfiles:
            proto_files.extend(dep[DefaultInfo].default_runfiles.files.to_list())
        if PyInfo in dep:
            for imp in dep[PyInfo].imports.to_list():
                path = "${{RUNFILES_DIR}}/{imp}".format(imp = imp)
                if path not in python_paths:
                    python_paths.append(path)

    runfiles = ctx.runfiles(files = all_files + [ctx.file._server_py] + proto_files)

    runner_script = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(
        output = runner_script,
        content = """#!/usr/bin/env bash
set -e

PYTHON_BIN="$(which python3 || which python)"
if [ -z "$PYTHON_BIN" ]; then
    echo "Error: Python 3 is required to run the Moosync dev registry." >&2
    exit 1
fi

SCRIPT_PATH="{server_py}"
if [ ! -f "$SCRIPT_PATH" ]; then
    if [ -n "$RUNFILES_DIR" ] && [ -f "$RUNFILES_DIR/_main/$SCRIPT_PATH" ]; then
        SCRIPT_PATH="$RUNFILES_DIR/_main/$SCRIPT_PATH"
    elif [ -n "$RUNFILES_DIR" ] && [ -f "$RUNFILES_DIR/moosync_ext/$SCRIPT_PATH" ]; then
        SCRIPT_PATH="$RUNFILES_DIR/moosync_ext/$SCRIPT_PATH"
    elif [ -n "$TEST_SRCDIR" ] && [ -f "$TEST_SRCDIR/_main/$SCRIPT_PATH" ]; then
        SCRIPT_PATH="$TEST_SRCDIR/_main/$SCRIPT_PATH"
    elif [ -n "$TEST_SRCDIR" ] && [ -f "$TEST_SRCDIR/moosync_ext/$SCRIPT_PATH" ]; then
        SCRIPT_PATH="$TEST_SRCDIR/moosync_ext/$SCRIPT_PATH"
    elif [ -f "$0.runfiles/_main/$SCRIPT_PATH" ]; then
        SCRIPT_PATH="$0.runfiles/_main/$SCRIPT_PATH"
    elif [ -f "$0.runfiles/moosync_ext/$SCRIPT_PATH" ]; then
        SCRIPT_PATH="$0.runfiles/moosync_ext/$SCRIPT_PATH"
    fi
fi

RUNFILES_DIR="${{RUNFILES_DIR:-$0.runfiles}}"
export PYTHONPATH="{python_paths}:${{PYTHONPATH}}"
echo "PYTHONPATH=$PYTHONPATH" >&2
exec "$PYTHON_BIN" "$SCRIPT_PATH" "$@"
""".format(
            server_py = ctx.file._server_py.short_path,
            python_paths = ":".join(python_paths),
        ),
        is_executable = True,
    )

    return [
        DefaultInfo(
            executable = runner_script,
            runfiles = runfiles,
        ),
    ]

dev_registry = rule(
    implementation = _dev_registry_impl,
    executable = True,
    attrs = {
        "extensions": attr.label_list(
            mandatory = True,
            doc = "List of extension targets to include in the dev registry",
        ),
        "_server_py": attr.label(
            default = "//tools/dev_registry:server.py",
            allow_single_file = True,
        ),
        "_proto_deps": attr.label_list(
            default = [
                "@extensions_sdk//protos:extensions_py_proto",
                "@extensions_sdk//protos:moosync_python_root",
            ],
        ),
    },
    doc = "Runs a local HTTP dev registry serving Protobuf manifests and .msxt packages.",
)
