# Moosync Extensions - IDE Setup Guide

This repository contains various extensions for Moosync, built hermetically using Bazel. To provide a seamless development experience with full Intellisense, code navigation, and error checking, configurations have been set up for both **VS Code** and **Zed**.

---

## 🛠️ Editor Configurations

The repository is pre-configured with:
- [`.vscode/settings.json`](.vscode/settings.json)
- [`.zed/settings.json`](.zed/settings.json)

These files configure language servers to look for targets in their respective subdirectories and exclude Bazel-generated build directories (`bazel-*`) to prevent IDE performance degradation.

---

## 🦀 Rust Extensions

All Rust extensions are built using `rules_rust` in Bazel targeting WebAssembly WASI (`wasm32-wasip1`). They are configured to run as individual standalone projects:
- **How it works:** Rust Analyzer loads each extension's `Cargo.toml` as an independent `linkedProject` and checks them using the `wasm32-wasip1` target.
- **Supported extensions:**
  - `discord`
  - `koel`
  - `lastfm`
  - `radio`
  - `spotify`
  - `youtube`
  - `sample_extensions/rs`
- **Setup:** None! Open the root of the workspace in VS Code or Zed, and the Rust Analyzer extension will automatically initialize and provide Intellisense.

## 🐹 Go Extensions

Go extensions are built using `rules_go` in Bazel.
- **How it works:** To ensure `gopls` accurately locates dependencies within the Bazel external cache, the workspace specifies a custom `GOPACKAGESDRIVER` located at [`tools/gopackagesdriver.sh`](tools/gopackagesdriver.sh).
- **Supported extensions:**
  - `soundcloud`
  - `sample_extensions/go`
- **Setup:**
  - **VS Code:** The settings in `.vscode/settings.json` automatically set `GOPACKAGESDRIVER` using `${workspaceFolder}`.
  - **Zed:** The settings in `.zed/settings.json` configure `gopls` to load `./tools/gopackagesdriver.sh`. If you experience resolution issues because of relative paths, update the env path in `.zed/settings.json` to be the absolute path to your `tools/gopackagesdriver.sh`.

---

## 🐍 Python Extensions

Python extensions use `aspect_rules_py` to run and compile code. You can generate virtual environments (`.venv`) directly inside each extension's directory using the following Bazel commands:

### Setup Commands

Run the following commands in the root of the repository to generate and link the virtual environments:

```bash
# 1. Generate venv for youtube_dl
bazel run //youtube_dl:youtube_dl_bin.venv -- --dest=$PWD/youtube_dl --name=.venv

# 2. Generate venv for sample_extensions/py
bazel run //sample_extensions/py:sample_extension_bin.venv -- --dest=$PWD/sample_extensions/py --name=.venv
```

The editor settings are pre-configured to automatically look at these generated directories (`youtube_dl/.venv` and `sample_extensions/py/.venv`) for autocomplete and module resolution.

---

## 🌐 JavaScript / TypeScript Extensions

JavaScript/TypeScript extensions use `@aspect_rules_js` for packaging.

To get local IDE Intellisense for the dependencies, make sure the `extensions-sdk` repository is cloned side-by-side with this repository (i.e. `../extensions-sdk`), and install the dependencies locally:

```bash
# From sample_extensions/js, install workspace dependencies
cd sample_extensions/js
pnpm install
```
pnpm will link the local workspace dependencies (like `wasm-extension-js`) from the side-by-side `extensions-sdk` repo.
