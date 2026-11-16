#!/usr/bin/env python3

import argparse
import io
import json
import mimetypes
import os
import sys
import zipfile
from http.server import HTTPServer, BaseHTTPRequestHandler
from typing import Dict, List, Optional, Tuple
from core.types.protos import extensions_pb2


def build_extension_manifest(ext_info: dict) -> extensions_pb2.ExtensionManifest:
    m = extensions_pb2.ExtensionManifest()
    if ext_info.get("pb_manifest") is not None:
        m.CopyFrom(ext_info["pb_manifest"])
    m.display_name = ext_info.get("display_name", "")
    if ext_info.get("extension_entry"):
        m.extension_entry = ext_info["extension_entry"]
    if ext_info.get("author"):
        m.author = ext_info["author"]
    m.name = ext_info.get("name", "")
    m.version = ext_info.get("version", "")
    if ext_info.get("icon"):
        m.icon = ext_info["icon"]
    if ext_info.get("description"):
        m.description = ext_info["description"]
    if ext_info.get("url"):
        m.url = ext_info["url"]
    if ext_info.get("registry"):
        m.registry = ext_info["registry"]
    return m


def build_registry_manifest(name: str, extensions: List[dict]) -> bytes:
    registry = extensions_pb2.ExtensionRegistryManifest()
    registry.name = name
    for ext in extensions:
        manifest_msg = build_extension_manifest(ext)
        registry.extensions.append(manifest_msg)
    return registry.SerializeToString()


class ExtensionRegistry:
    def __init__(self, registry_name: str):
        self.registry_name = registry_name
        self.extensions: List[dict] = []
        self.file_map: Dict[str, bytes] = {}
        self.manifest_pb: bytes = b""
        self.manifest_json: str = "{}"

    def discover_extensions(self, search_paths: List[str]):
        seen_packages = set()
        msxt_files = []

        for base_path in search_paths:
            if not os.path.exists(base_path):
                continue
            for root, _, files in os.walk(base_path):
                for f in files:
                    if f.endswith(".msxt"):
                        msxt_files.append(os.path.join(root, f))

        for msxt_path in msxt_files:
            try:
                with open(msxt_path, "rb") as f:
                    msxt_bytes = f.read()

                with zipfile.ZipFile(io.BytesIO(msxt_bytes)) as zf:
                    pb_manifest = None
                    pkg_name = None
                    disp_name = None
                    version = "0.0.1"
                    desc = None
                    author = None
                    icon_rel = None

                    if "manifest.pb" in zf.namelist():
                        with zf.open("manifest.pb") as pf:
                            pb_manifest = extensions_pb2.ExtensionManifest.FromString(pf.read())
                            pkg_name = pb_manifest.name
                            disp_name = pb_manifest.display_name or pb_manifest.name
                            version = pb_manifest.version or "0.0.1"
                            desc = pb_manifest.description
                            author = pb_manifest.author
                            icon_rel = pb_manifest.icon

                    if not pkg_name:
                        pkg_name = os.path.splitext(os.path.basename(msxt_path))[0]

                    if pkg_name in seen_packages:
                        continue
                    seen_packages.add(pkg_name)

                    if not disp_name:
                        disp_name = pkg_name

                    filename = f"{pkg_name}.msxt"
                    self.file_map[filename] = msxt_bytes
                    orig_filename = os.path.basename(msxt_path)
                    self.file_map[orig_filename] = msxt_bytes

                    icon_url = None
                    if icon_rel and icon_rel in zf.namelist():
                        icon_bytes = zf.read(icon_rel)
                        icon_serve_path = f"icons/{pkg_name}/{os.path.basename(icon_rel)}"
                        self.file_map[icon_serve_path] = icon_bytes
                        icon_url = icon_serve_path

                    ext_info = {
                        "pb_manifest": pb_manifest,
                        "display_name": disp_name,
                        "extension_entry": pb_manifest.extension_entry if pb_manifest else "",
                        "author": author,
                        "name": pkg_name,
                        "version": version,
                        "icon": icon_url or "",
                        "description": desc,
                        "url": filename,
                        "registry": self.registry_name,
                    }
                    self.extensions.append(ext_info)
            except Exception as e:
                print(f"[WARN] Failed to process {msxt_path}: {e}", file=sys.stderr)

        self.manifest_pb = build_registry_manifest(
            self.registry_name, self.extensions
        )
        self.manifest_json = json.dumps(
            {
                "name": self.registry_name,
                "extensions": {
                    ext["name"]: {
                        "displayName": ext["display_name"],
                        "version": ext["version"],
                        "url": ext["url"],
                        "icon": ext["icon"],
                        "description": ext["description"],
                    }
                    for ext in self.extensions
                },
            },
            indent=2,
        )


def make_handler(registry: ExtensionRegistry):
    class RegistryHTTPHandler(BaseHTTPRequestHandler):
        def log_message(self, format, *args):
            print(f"[HTTP] {self.address_string()} - {format % args}")

        def do_OPTIONS(self):
            self.send_response(204)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Access-Control-Allow-Methods", "GET, OPTIONS")
            self.send_header("Access-Control-Allow-Headers", "*")
            self.end_headers()

        def do_HEAD(self):
            self.do_GET()

        def do_GET(self):
            path = self.path.split("?")[0].lstrip("/")

            if path in ("", "index.html"):
                self.serve_dashboard()
                return

            if path in ("manifest.pb", "manifest.bin", "manifest"):
                self.send_response(200)
                self.send_header("Content-Type", "application/x-protobuf")
                self.send_header("Access-Control-Allow-Origin", "*")
                self.send_header("Content-Length", str(len(registry.manifest_pb)))
                self.end_headers()
                self.wfile.write(registry.manifest_pb)
                return

            if path == "manifest.json":
                json_bytes = registry.manifest_json.encode("utf-8")
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Access-Control-Allow-Origin", "*")
                self.send_header("Content-Length", str(len(json_bytes)))
                self.end_headers()
                self.wfile.write(json_bytes)
                return

            if path in registry.file_map:
                file_bytes = registry.file_map[path]
                mime, _ = mimetypes.guess_type(path)
                if not mime:
                    mime = "application/octet-stream"
                    if path.endswith(".msxt"):
                        mime = "application/zip"

                self.send_response(200)
                self.send_header("Content-Type", mime)
                self.send_header("Access-Control-Allow-Origin", "*")
                self.send_header("Content-Length", str(len(file_bytes)))
                self.end_headers()
                self.wfile.write(file_bytes)
                return

            self.send_response(404)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"404 Not Found")

        def serve_dashboard(self):
            host_header = self.headers.get("Host", "localhost:8080")
            base_url = f"http://{host_header}"

            items_html = ""
            for ext in registry.extensions:
                icon_tag = f'<img src="/{ext["icon"]}" width="32" height="32" style="vertical-align:middle;margin-right:10px;"/>' if ext["icon"] else ""
                desc = ext["description"] or "No description provided."
                items_html += f"""
                <div style="border:1px solid #333;border-radius:8px;padding:16px;margin-bottom:12px;background:#1e1e1e;">
                    <div style="display:flex;align-items:center;justify-content:space-between;">
                        <div>
                            <h3 style="margin:0;color:#fff;">{icon_tag}{ext['display_name']} <span style="font-size:12px;color:#888;">v{ext['version']}</span></h3>
                            <p style="margin:4px 0 0 0;color:#aaa;font-family:monospace;font-size:12px;">{ext['name']}</p>
                            <p style="margin:8px 0 0 0;color:#ccc;">{desc}</p>
                        </div>
                        <a href="/{ext['url']}" style="background:#007acc;color:#fff;text-decoration:none;padding:8px 16px;border-radius:4px;font-weight:bold;">Download .msxt</a>
                    </div>
                </div>
                """

            html = f"""<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>{registry.registry_name}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #121212; color: #e0e0e0; margin: 40px auto; max-width: 800px; padding: 0 20px; }}
        code {{ background: #222; padding: 3px 6px; border-radius: 4px; font-family: monospace; color: #4ec9b0; }}
        .header {{ border-bottom: 1px solid #333; padding-bottom: 20px; margin-bottom: 24px; }}
        .registry-url {{ background: #252526; border: 1px solid #3c3c3c; border-radius: 6px; padding: 12px; font-family: monospace; font-size: 14px; color: #ce9178; word-break: break-all; margin: 12px 0; }}
    </style>
</head>
<body>
    <div class="header">
        <h1 style="margin:0;color:#fff;">{registry.registry_name}</h1>
        <p style="color:#aaa;">Local development extension registry for Moosync.</p>
        <p>To use this registry in Moosync, add the following URL in <b>Settings &rarr; Extensions &rarr; Registries</b>:</p>
        <div class="registry-url">{base_url}/manifest.pb</div>
        <p><a href="/manifest.pb" style="color:#3794ff;">Download manifest.pb (Protobuf)</a> | <a href="/manifest.json" style="color:#3794ff;">View manifest.json</a></p>
    </div>
    <h2>Registered Extensions ({len(registry.extensions)})</h2>
    {items_html or '<p style="color:#888;">No extensions found.</p>'}
</body>
</html>
"""
            body = html.encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    return RegistryHTTPHandler


def main():
    parser = argparse.ArgumentParser(description="Moosync Dev Registry Server")
    parser.add_argument("--port", type=int, default=8080, help="Port to listen on (default: 8080)")
    parser.add_argument("--host", type=str, default="0.0.0.0", help="Host interface to bind to (default: 0.0.0.0)")
    parser.add_argument("--name", type=str, default="Moosync Dev Registry", help="Registry name")
    parser.add_argument("--scan-dirs", nargs="*", default=[], help="Extra directories to scan for .msxt files")
    args = parser.parse_args()

    search_paths = list(args.scan_dirs)
    runfiles_dir = os.environ.get("PYTHON_RUNFILES") or os.environ.get("TEST_SRCDIR") or os.environ.get("BUILD_WORKSPACE_DIRECTORY")
    if runfiles_dir:
        search_paths.append(runfiles_dir)
    search_paths.append(os.getcwd())
    if "BUILD_WORKING_DIRECTORY" in os.environ:
        search_paths.append(os.environ["BUILD_WORKING_DIRECTORY"])

    registry = ExtensionRegistry(args.name)
    registry.discover_extensions(search_paths)

    HTTPServer.allow_reuse_address = True
    handler_class = make_handler(registry)
    server = HTTPServer((args.host, args.port), handler_class)

    url = f"http://localhost:{args.port}/manifest.pb"
    print("=" * 80)
    print(f" Moosync Dev Registry is running at:")
    print(f"   {url}")
    print("=" * 80)
    print(f" Registered Extensions ({len(registry.extensions)}):")
    for ext in registry.extensions:
        print(f"   * {ext['display_name']} ({ext['name']} v{ext['version']}) -> http://localhost:{args.port}/{ext['url']}")
    print("=" * 80)
    print(" In Moosync, open Settings -> Extensions -> Registries and add:")
    print(f"   {url}")
    print("=" * 80)
    print(" Press Ctrl+C to stop the server.\n")

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopping Moosync Dev Registry server.")
        server.server_close()


if __name__ == "__main__":
    main()
