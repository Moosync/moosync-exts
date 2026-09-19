#!/usr/bin/env python3
"""
Moosync Dev Registry Server
Scans registered extension packages (.msxt), generates binary Protobuf manifests (manifest.pb),
and serves a local HTTP registry for Moosync development and testing.
"""

import argparse
import io
import json
import mimetypes
import os
import sys
import zipfile
from http.server import HTTPServer, BaseHTTPRequestHandler
from typing import Dict, List, Optional, Tuple


def encode_varint(value: int) -> bytes:
    buf = bytearray()
    while value > 0x7F:
        buf.append((value & 0x7F) | 0x80)
        value >>= 7
    buf.append(value & 0x7F)
    return bytes(buf)


def encode_string(field_num: int, value: str) -> bytes:
    if not value:
        return b""
    encoded = value.encode("utf-8")
    tag = encode_varint((field_num << 3) | 2)
    return tag + encode_varint(len(encoded)) + encoded


def encode_fetched_manifest(manifest: dict) -> bytes:
    """Encodes a FetchedExtensionManifest protobuf message."""
    out = bytearray()
    out.extend(encode_string(1, manifest.get("name", "")))
    out.extend(encode_string(2, manifest.get("package_name", "")))
    if manifest.get("logo"):
        out.extend(encode_string(3, manifest["logo"]))
    if manifest.get("description"):
        out.extend(encode_string(4, manifest["description"]))
    out.extend(encode_string(5, manifest.get("url", "")))
    out.extend(encode_string(6, manifest.get("version", "")))
    if manifest.get("registry"):
        out.extend(encode_string(7, manifest["registry"]))
    return bytes(out)


def encode_registry_manifest(name: str, extensions: List[dict]) -> bytes:
    """Encodes an ExtensionRegistryManifest protobuf message."""
    out = bytearray()
    out.extend(encode_string(1, name))
    for ext in extensions:
        ext_bytes = encode_fetched_manifest(ext)
        tag = encode_varint((2 << 3) | 2)
        out.extend(tag + encode_varint(len(ext_bytes)) + ext_bytes)
    return bytes(out)


class ExtensionRegistry:
    def __init__(self, registry_name: str):
        self.registry_name = registry_name
        self.extensions: List[dict] = []
        self.file_map: Dict[str, bytes] = {}
        self.manifest_pb: bytes = b""
        self.manifest_json: str = "{}"

    def discover_extensions(self, search_paths: List[str]):
        """Finds all .msxt files and extracts metadata."""
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
                    pkg_json_data = {}
                    if "package.json" in zf.namelist():
                        with zf.open("package.json") as pf:
                            pkg_json_data = json.load(pf)

                    pkg_name = pkg_json_data.get("name")
                    if not pkg_name:
                        pkg_name = os.path.splitext(os.path.basename(msxt_path))[0]

                    if pkg_name in seen_packages:
                        continue
                    seen_packages.add(pkg_name)

                    disp_name = pkg_json_data.get("displayName") or pkg_name
                    version = pkg_json_data.get("version") or "0.0.1"
                    desc = pkg_json_data.get("description") or pkg_json_data.get("desc")

                    filename = f"{pkg_name}.msxt"
                    self.file_map[filename] = msxt_bytes
                    # Also map original filename if different
                    orig_filename = os.path.basename(msxt_path)
                    self.file_map[orig_filename] = msxt_bytes

                    icon_url = None
                    icon_rel = pkg_json_data.get("icon")
                    if icon_rel and icon_rel in zf.namelist():
                        icon_bytes = zf.read(icon_rel)
                        icon_serve_path = f"icons/{pkg_name}/{os.path.basename(icon_rel)}"
                        self.file_map[icon_serve_path] = icon_bytes
                        icon_url = icon_serve_path

                    ext_info = {
                        "name": disp_name,
                        "package_name": pkg_name,
                        "version": version,
                        "url": filename,
                        "logo": icon_url,
                        "description": desc,
                        "registry": self.registry_name,
                    }
                    self.extensions.append(ext_info)
            except Exception as e:
                print(f"[WARN] Failed to process {msxt_path}: {e}", file=sys.stderr)

        self.manifest_pb = encode_registry_manifest(
            self.registry_name, self.extensions
        )
        self.manifest_json = json.dumps(
            {
                "name": self.registry_name,
                "extensions": {
                    ext["package_name"]: {
                        "displayName": ext["name"],
                        "version": ext["version"],
                        "url": ext["url"],
                        "icon": ext["logo"],
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
                icon_tag = f'<img src="/{ext["logo"]}" width="32" height="32" style="vertical-align:middle;margin-right:10px;"/>' if ext["logo"] else ""
                desc = ext["description"] or "No description provided."
                items_html += f"""
                <div style="border:1px solid #333;border-radius:8px;padding:16px;margin-bottom:12px;background:#1e1e1e;">
                    <div style="display:flex;align-items:center;justify-content:space-between;">
                        <div>
                            <h3 style="margin:0;color:#fff;">{icon_tag}{ext['name']} <span style="font-size:12px;color:#888;">v{ext['version']}</span></h3>
                            <p style="margin:4px 0 0 0;color:#aaa;font-family:monospace;font-size:12px;">{ext['package_name']}</p>
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

    # Determine runfiles and scan directories
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
        print(f"   * {ext['name']} ({ext['package_name']} v{ext['version']}) -> http://localhost:{args.port}/{ext['url']}")
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
