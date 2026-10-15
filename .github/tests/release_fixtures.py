"""Generated, credential-free fixtures for release controller contracts.

Never read a downloaded archive, invoke Cargo, or contact a registry here.
"""
from __future__ import annotations

import copy
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile

VERSION = "0.0.1"
OLD_SOURCE = "a2c59ef1cedfee120762a4e7727b62224e57a8e7"
OLD_TREE = "66deea4bc3e0a490eda8e08f4392dead3f62c177"
OLD_TAG = "bd1d904ef84f6f0bc6f2b361de9bc6931bc74fa0"
NEW_SOURCE = "1" * 40
NEW_TREE = "2" * 40
NEW_TAG = "3" * 40
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
REPOSITORY = "https://github.com/EffortlessMetrics/faultline-rs"
ANCHORS = {"faultline-codes", "faultline-types", "faultline-junit", "faultline-loader", "faultline-localization"}
FRESH = {"faultline-rs", "faultline-app", "faultline-git", "faultline-ports", "faultline-probe-exec", "faultline-render", "faultline-sarif", "faultline-store", "faultline-surface"}
PUBLIC = ANCHORS | FRESH
ORDER = ["faultline-loader", "faultline-codes", "faultline-types", "faultline-localization", "faultline-junit", "faultline-surface", "faultline-ports", "faultline-app", "faultline-git", "faultline-probe-exec", "faultline-store", "faultline-render", "faultline-sarif", "faultline-rs"]
NEW_README = b"# Generated faultline-rs landing page\nNew package and executable: faultline-rs\n"
OLD_README = b"# Generated original release landing page\nOriginal immutable anchor\n"
LICENSE = b"Generated Apache License fixture\nEND OF TERMS AND CONDITIONS\n"
# Types' root-dev loader must appear in types' own package lock, but not in
# another root's closure merely because that root normally depends on types.
NORMAL = {
    "faultline-types": ["faultline-codes"],
    "faultline-localization": ["faultline-types"],
    "faultline-junit": ["faultline-types"],
    "faultline-surface": ["faultline-localization"],
    "faultline-ports": ["faultline-types"],
    "faultline-app": ["faultline-surface", "faultline-ports"],
    "faultline-git": ["faultline-ports"],
    "faultline-probe-exec": ["faultline-ports"],
    "faultline-store": ["faultline-ports"],
    "faultline-render": ["faultline-types"],
    "faultline-sarif": ["faultline-types"],
    "faultline-rs": ["faultline-app", "faultline-git", "faultline-probe-exec", "faultline-store", "faultline-render", "faultline-sarif"],
}
DEV = {"faultline-types": ["faultline-loader"], "faultline-app": ["faultline-junit"]}
BUILD = {"faultline-rs": ["faultline-localization"]}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def closure(name, normal=None, dev=None, build=None):
    normal = NORMAL if normal is None else normal
    dev = DEV if dev is None else dev
    build = BUILD if build is None else build
    found = set(normal.get(name, ())) | set(dev.get(name, ())) | set(build.get(name, ()))
    pending = list(found)
    while pending:
        child = pending.pop()
        for dependency in list(normal.get(child, ())) + list(build.get(child, ())):
            if dependency not in found:
                found.add(dependency)
                pending.append(dependency)
    return found


def toml_lock(records):
    lines = ["version = 4\n"]
    for record in records:
        lines.append("\n[[package]]\n")
        for key, value in record.items():
            lines.append(f"{key} = {json.dumps(value)}\n")
    return "".join(lines).encode()


def archive_bytes(name, files):
    stream = io.BytesIO()
    with tarfile.open(fileobj=stream, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        for path, data in sorted(files.items()):
            member = tarfile.TarInfo(f"{name}-{VERSION}/{path}")
            member.size = len(data)
            member.mode = 0o644
            member.mtime = 0
            archive.addfile(member, io.BytesIO(data))
    return gzip.compress(stream.getvalue(), mtime=0)


def unpack_archive(data):
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        return {member.name.split("/", 1)[1]: archive.extractfile(member).read()
                for member in archive.getmembers() if member.isfile()}


class SyntheticRelease:
    """Build hash-linked archives in topological order, with mixed provenance."""
    def __init__(self):
        self.archives = {}
        self.files = {}
        self.locks = {}
        self.manifests = {}
        self.build()

    def manifest(self, name):
        fields = {"name": name, "version": VERSION, "license": "Apache-2.0", "rust-version": "1.93", "description": "Generated test fixture", "repository": REPOSITORY, "readme": "README.md"}
        lines = ["[package]\n"] + [f"{key} = {json.dumps(value)}\n" for key, value in fields.items()]
        if name == "faultline-rs":
            lines += ['\n[[bin]]\nname = "faultline-rs"\npath = "src/main.rs"\n']
        for table, graph in [("dependencies", NORMAL), ("build-dependencies", BUILD), ("dev-dependencies", DEV)]:
            if graph.get(name):
                lines.append(f"\n[{table}]\n")
                lines += [f'{dependency} = "{VERSION}"\n' for dependency in graph[name]]
        return "".join(lines).encode()

    def build(self, changes=None):
        changes = changes or {}
        for name in ORDER:
            dependencies = sorted(closure(name))
            records = [{"name": name, "version": VERSION}]
            records += [{"name": dependency, "version": VERSION, "source": REGISTRY, "checksum": sha(self.archives[dependency])} for dependency in dependencies]
            for record in records:
                dependency_name = record["name"]
                edges = set(NORMAL.get(dependency_name, ())) | set(BUILD.get(dependency_name, ()))
                if dependency_name == name:
                    edges.update(DEV.get(name, ()))
                if edges:
                    record["dependencies"] = [f"{edge} {VERSION}" for edge in sorted(edges)]
            # An ordinary registry dependency demonstrates that internal closure
            # enforcement need not reject legitimate external lock entries.
            records.append({"name": "generated-external", "version": "1.0.0", "source": REGISTRY, "checksum": "a" * 64})
            manifest = self.manifest(name)
            original = manifest.replace(b'\n[[bin]]\nname = "faultline-rs"\npath = "src/main.rs"\n', b'') if name == "faultline-rs" else manifest
            files = {"Cargo.toml": manifest, "Cargo.toml.orig": original,
                     "Cargo.lock": toml_lock(records),
                     "README.md": OLD_README if name in ANCHORS else NEW_README,
                     "LICENSE": LICENSE, "src/main.rs" if name == "faultline-rs" else "src/lib.rs": b"// Generated fixture only\n",
                     ".cargo_vcs_info.json": json.dumps({"git": {"sha1": OLD_SOURCE if name in ANCHORS else NEW_SOURCE, "dirty": False}, "path_in_vcs": "crates/faultline-cli" if name == "faultline-rs" else "crates/" + name}).encode()}
            if name in changes:
                changes[name](files, records)
                files["Cargo.lock"] = toml_lock(records)
            self.manifests[name] = original
            self.files[name] = copy.deepcopy(files)
            self.locks[name] = copy.deepcopy(records)
            self.archives[name] = archive_bytes(name, files)
        return self

    def write(self, root):
        root = Path(root)
        package = root / "target/package"
        package.mkdir(parents=True, exist_ok=True)
        (root / "README.md").write_bytes(NEW_README)
        (root / "LICENSE").write_bytes(LICENSE)
        members = ["crates/faultline-cli" if name == "faultline-rs" else "crates/" + name for name in ORDER]
        members += ["crates/faultline-fixtures", "crates/xtask"]
        (root / "Cargo.toml").write_text("[workspace]\nmembers = " + json.dumps(members) + '\n[workspace.package]\nversion = "0.0.1"\nlicense = "Apache-2.0"\nrust-version = "1.93"\nrepository = "' + REPOSITORY + '"\n')
        for name in ORDER:
            source = root / ("crates/faultline-cli" if name == "faultline-rs" else "crates/" + name)
            source.mkdir(parents=True, exist_ok=True)
            (source / "Cargo.toml").write_bytes(self.manifests[name])
            (source / "src").mkdir(exist_ok=True)
            (source / ("src/main.rs" if name == "faultline-rs" else "src/lib.rs")).write_bytes(b"// Generated fixture only\n")
        for name in ("faultline-fixtures", "xtask"):
            source = root / "crates" / name
            source.mkdir(parents=True, exist_ok=True)
            (source / "Cargo.toml").write_text('[package]\nname = "' + name + '"\nversion = "0.0.1"\npublish = false\n')
        for name, data in self.archives.items():
            (package / f"{name}-{VERSION}.crate").write_bytes(data)
        return package
