import argparse
import os
import sys
import zipfile

try:
    from core.types.protos import extensions_pb2
except ImportError:
    import extensions_pb2


def main():
    parser = argparse.ArgumentParser(description="Generate ExtensionRegistryManifest manifest.pb")
    parser.add_argument("--out", required=True, help="Output manifest.pb path")
    parser.add_argument("extensions", nargs="+", help="Input .msxt files")
    args = parser.parse_args()

    registry_manifest = extensions_pb2.ExtensionRegistryManifest()
    registry_manifest.name = "Moosync Extensions"

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

                pkg_name = pb_manifest.name or os.path.splitext(os.path.basename(msxt_path))[0]
                if pkg_name in seen_packages:
                    continue
                seen_packages.add(pkg_name)

                if not pb_manifest.display_name:
                    pb_manifest.display_name = pkg_name
                if not pb_manifest.version:
                    pb_manifest.version = "0.0.1"
                pb_manifest.url = f"{pkg_name}.msxt"
                pb_manifest.registry = "Moosync Extensions"

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
