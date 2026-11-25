"""Hermetically extracts .so files from termux-pacman glibc packages, aligns to 16 KiB, and packs a .tar.gz."""

import argparse
import io
import tarfile

from tools.elf_align import align_elf_16k

TERMUX_GLIBC_LIB_DIR = "data/data/com.termux/files/usr/glibc/lib"
WANTED_LIBS = ("libc.so.6", "libm.so.6", "libatomic.so.1", "libgcc_s.so.1")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out_path", help="Output .tar.gz path")
    parser.add_argument("extra_so", help="Path to libpipewire-0.3.so.0 or '-'")
    parser.add_argument("pkg_paths", nargs="+", help="Termux pacman .tar.xz archives")
    args = parser.parse_args()

    wanted = {f"{TERMUX_GLIBC_LIB_DIR}/{name}": name for name in WANTED_LIBS}
    collected: dict[str, bytes] = {}

    for pkg in args.pkg_paths:
        with tarfile.open(pkg, "r:xz") as tf:
            members = {m.name: m for m in tf.getmembers()}
            for full_name, out_name in wanted.items():
                if full_name not in members:
                    continue
                member = members[full_name]
                if member.issym():
                    target_name = f"{TERMUX_GLIBC_LIB_DIR}/{member.linkname.lstrip('/')}"
                    if target_name in members:
                        member = members[target_name]
                fobj = tf.extractfile(member)
                if fobj is not None:
                    collected[out_name] = align_elf_16k(fobj.read())

    with tarfile.open(args.out_path, "w:gz") as out_tar:
        for name, content in sorted(collected.items()):
            info = tarfile.TarInfo(name=name)
            info.size = len(content)
            info.mode = 0o755
            out_tar.addfile(info, io.BytesIO(content))
        if args.extra_so != "-":
            with open(args.extra_so, "rb") as f:
                content = align_elf_16k(f.read())
            info = tarfile.TarInfo(name="libpipewire-0.3.so.0")
            info.size = len(content)
            info.mode = 0o755
            out_tar.addfile(info, io.BytesIO(content))


if __name__ == "__main__":
    main()
