#!/usr/bin/env python3
"""Prepare the fixed 5.4 local source-bundle compatibility review; never publish."""
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile
import tempfile

SOURCE_PIN = "85a9743023de788d91a370601ad2b7d5a649517a"
ENGINE_PIN = "b0e4b445e1a20a9354d55ebb41bbedaf16ec8a74"
ENGINE_SHA256 = "6fa49f998a23803c4df4c47e9b7f55de32cb867e923364f711b6694980600d45"
LIMIT = 64 * 1024 * 1024


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def extract(raw, destination, prefix=None):
    with tarfile.open(fileobj=io.BytesIO(raw), mode="r:*") as archive:
        members = archive.getmembers()
        if len(members) > 10000 or sum(m.size for m in members) > LIMIT:
            raise ValueError("archive extraction budget")
        seen = set()
        for member in members:
            path = PurePosixPath(member.name)
            if path.is_absolute() or ".." in path.parts or "\\" in member.name or not path.parts:
                raise ValueError("unsafe archive path")
            if prefix and path.parts[0] != prefix:
                raise ValueError("engine prefix mismatch")
            if member.name in seen or not (member.isfile() or member.isdir()):
                raise ValueError("duplicate or non-regular archive member")
            seen.add(member.name)
        archive.extractall(destination, members=members, filter="data")


def prepare(engine_path, destination):
    engine_path, destination = Path(engine_path), Path(destination)
    if destination.exists() or destination.is_symlink():
        raise ValueError("destination exists; retained artifacts are immutable")
    if engine_path.stat().st_size > LIMIT:
        raise ValueError("engine archive budget")
    engine = engine_path.read_bytes()
    if sha(engine) != ENGINE_SHA256:
        raise ValueError("engine archive differs from fixed reviewed source bundle")
    repository = Path(__file__).resolve().parent.parent
    source = subprocess.check_output(["/usr/bin/git", "-C", str(repository), "archive", "--format=tar", SOURCE_PIN])
    if len(source) > LIMIT:
        raise ValueError("source archive budget")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="gitguard-local-package-", dir=destination.parent) as temporary:
        root = Path(temporary)
        (root / "gitguard").mkdir()
        extract(source, root / "gitguard")
        extract(engine, root, "guardengine-0.1.0")
        manifest = root / "gitguard/Cargo.toml"
        original = manifest.read_text()
        if original.count('path = "../guardengine"') != 1:
            raise ValueError("unsupported manifest transform")
        manifest.write_text(original.replace('path = "../guardengine"', 'path = "../guardengine-0.1.0"'))
        (root / "gitguard-source.tar").write_bytes(source)
        (root / "guardengine-source.tar").write_bytes(engine)
        provenance = {
            "profile": "gitguard.local-source-compatibility/v1", "source_commit": SOURCE_PIN,
            "source_archive_sha256": sha(source), "engine_source_commit": ENGINE_PIN,
            "engine_archive_sha256": ENGINE_SHA256, "engine_artifact_kind": "source_bundle",
            "manifest_sha256": sha(manifest.read_bytes()),
            "lock_sha256": sha((root / "gitguard/Cargo.lock").read_bytes()),
            "manifest_transform": "only engine path changed to bundled source",
            "published": False, "compatibility_verified": False,
        }
        (root / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
        shutil.copytree(root, destination)
    return provenance


if __name__ == "__main__":
    try:
        if len(sys.argv) != 3:
            raise ValueError("expected REVIEWED_GE_SOURCE_BUNDLE NEW_OUTPUT_DIRECTORY")
        print(json.dumps(prepare(*sys.argv[1:]), sort_keys=True))
    except (OSError, ValueError, subprocess.SubprocessError, tarfile.TarError) as error:
        print(json.dumps({"error": str(error), "published": False}), file=sys.stderr)
        sys.exit(4)
