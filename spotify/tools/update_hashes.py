"""Bazel tool to regenerate soloist, soloist_android_x86_64, and aaudio_sink hashes in spotify/BUILD."""

import argparse
import hashlib
import io
import os
import re
import tarfile
import urllib.request

from tools.elf_align import align_elf_16k

SOLOIST_X86_64_URL = "https://soloist-builds.spotifycdn.com/soloist_release_x86_64.tar.gz"


def compute_hashes(data: bytes) -> dict[str, str]:
    return {
        "sha256": hashlib.sha256(data).hexdigest(),
        "sha512": hashlib.sha512(data).hexdigest(),
        "md5": hashlib.md5(data).hexdigest(),
    }


def fetch_soloist_binary(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=60) as resp:
        archive_bytes = resp.read()
    with tarfile.open(fileobj=io.BytesIO(archive_bytes), mode="r:gz") as tf:
        for member in tf.getmembers():
            if member.isfile() and (member.name == "soloist" or member.name.endswith("/soloist")):
                fobj = tf.extractfile(member)
                if fobj is not None:
                    return fobj.read()
    raise RuntimeError(f"soloist binary not found in {url}")


def update_binary_block(build_text: str, name: str, hashes: dict[str, str]) -> str:
    pattern = re.compile(
        rf'("{re.escape(name)}"\s*:\s*\{{.*?"sha256"\s*:\s*")[^"]*(".*?"sha512"\s*:\s*")[^"]*(".*?"md5"\s*:\s*")[^"]*(")',
        re.DOTALL,
    )
    updated, count = pattern.subn(
        rf'\g<1>{hashes["sha256"]}\g<2>{hashes["sha512"]}\g<3>{hashes["md5"]}\g<4>',
        build_text,
        count=1,
    )
    if count != 1:
        raise RuntimeError(f"Failed to update hash block for '{name}' in BUILD")
    return updated


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aaudio-sink", required=True, help="Path to built aaudio_sink binary")
    parser.add_argument("--url", default=SOLOIST_X86_64_URL, help="Soloist release tar.gz URL")
    args = parser.parse_args()

    ws = os.environ.get("BUILD_WORKSPACE_DIRECTORY")
    if not ws:
        raise RuntimeError("Run this tool via `bazel run //spotify:update_hashes`")
    build_path = os.path.join(ws, "spotify", "BUILD")

    soloist_raw = fetch_soloist_binary(args.url)
    soloist_android = align_elf_16k(soloist_raw)
    with open(args.aaudio_sink, "rb") as f:
        aaudio_raw = f.read()

    updates = {
        "soloist": compute_hashes(soloist_raw),
        "soloist_android_x86_64": compute_hashes(soloist_android),
        "aaudio_sink": compute_hashes(aaudio_raw),
    }

    with open(build_path, "r", encoding="utf-8") as f:
        text = f.read()
    for name, hashes in updates.items():
        text = update_binary_block(text, name, hashes)
    with open(build_path, "w", encoding="utf-8") as f:
        f.write(text)

    for name, hashes in updates.items():
        print(f"{name}: sha256={hashes['sha256']}")


if __name__ == "__main__":
    main()
