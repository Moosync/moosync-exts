#!/usr/bin/env bash
export GOPACKAGESDRIVER_BAZEL_KINDS="go_non_executable_binary"
exec bazel run -- @rules_go//go/tools/gopackagesdriver "${@}"