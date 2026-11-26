import argparse
import hashlib
import os
import subprocess
import sys
import tempfile
import zipfile

try:
    from core.types.protos import extensions_pb2
except ImportError:
    import extensions_pb2


def compute_sha256(file_path: str) -> str:
    h = hashlib.sha256()
    with open(file_path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def export_public_key(signing_key_path: str) -> bytes:
    with tempfile.TemporaryDirectory(prefix="moosync_reg_gpg_") as gnupghome:
        os.chmod(gnupghome, 0o700)
        env = os.environ.copy()
        env["GNUPGHOME"] = gnupghome

        res = subprocess.run(
            ["gpg", "--batch", "--yes", "--import", signing_key_path],
            env=env,
            capture_output=True,
        )
        if res.returncode != 0:
            raise RuntimeError(f"gpg --import failed: {res.stderr.decode(errors='replace')}")

        exp = subprocess.run(
            ["gpg", "--batch", "--yes", "--armor", "--export"],
            env=env,
            capture_output=True,
        )
        if exp.returncode != 0 or not exp.stdout:
            raise RuntimeError(f"gpg --export failed: {exp.stderr.decode(errors='replace')}")
        return exp.stdout


def main():
    parser = argparse.ArgumentParser(description="Generate ExtensionRegistryManifest manifest.pb")
    parser.add_argument("--out", required=True, help="Output manifest.pb path")
    parser.add_argument("--signing-key", default="", help="Optional OpenPGP private key to export public_key")
    parser.add_argument("--public-key", default="", help="Optional OpenPGP public key file")
    parser.add_argument("extensions", nargs="+", help="Input .msxt files")
    args = parser.parse_args()

    registry_manifest = extensions_pb2.ExtensionRegistryManifest()
    registry_manifest.name = "Moosync Extensions"

    if args.public_key and os.path.isfile(args.public_key) and os.path.getsize(args.public_key) > 0:
        with open(args.public_key, "rb") as pkf:
            registry_manifest.public_key = pkf.read()
    elif args.signing_key and os.path.isfile(args.signing_key) and os.path.getsize(args.signing_key) > 0:
        registry_manifest.public_key = export_public_key(args.signing_key)

    seen_packages = set()

    for msxt_path in sorted(args.extensions):
        if not msxt_path.endswith(".msxt"):
            continue

        try:
            with zipfile.ZipFile(msxt_path, "r") as zf:
                if "manifest.pb" not in zf.namelist():
                    print(f"Warning: 'manifest.pb' missing in {msxt_path}", file=sys.stderr)
                    continue

                with zf.open("manifest.pb") as pf:
                    pb_manifest = extensions_pb2.ExtensionManifest.FromString(pf.read())

                pkg_name = pb_manifest.package_name or os.path.splitext(os.path.basename(msxt_path))[0]
                if pkg_name in seen_packages:
                    continue
                seen_packages.add(pkg_name)

                if not pb_manifest.display_name:
                    pb_manifest.display_name = pkg_name
                if not pb_manifest.version:
                    pb_manifest.version = "0.0.1"
                pb_manifest.url = f"{pkg_name}.msxt"
                pb_manifest.registry = "Moosync Extensions"
                pb_manifest.sha256 = compute_sha256(msxt_path)

                registry_manifest.extensions.append(pb_manifest)
        except Exception as e:
            print(f"Error processing {msxt_path}: {e}", file=sys.stderr)
            sys.exit(1)

    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    with open(args.out, "wb") as f:
        f.write(registry_manifest.SerializeToString())
    print(f"Binary manifest successfully written to {args.out}")


if __name__ == "__main__":
    main()
