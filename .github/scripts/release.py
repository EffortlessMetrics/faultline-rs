"""Scoped faultline-rs 0.0.1 qualification and publication controls (Python 3.11+)."""

import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import tempfile
import time
import tomllib
import urllib.error
import urllib.request
import zlib

if not __debug__:
    raise RuntimeError("Release controls require Python assertions")


REPOSITORY = "https://github.com/EffortlessMetrics/faultline-rs"
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
SUPPORTS = {"faultline-app", "faultline-codes", "faultline-git", "faultline-junit",
            "faultline-loader", "faultline-localization", "faultline-ports",
            "faultline-probe-exec", "faultline-render", "faultline-sarif",
            "faultline-store", "faultline-surface", "faultline-types"}
PUBLISHED = {"faultline-codes", "faultline-types", "faultline-junit",
             "faultline-loader", "faultline-localization"}
PUBLIC = SUPPORTS | {"faultline-rs"}
PRIVATE = {"faultline-fixtures", "xtask"}
LEGACY = {"sha": "a2c59ef1cedfee120762a4e7727b62224e57a8e7",
          "tree": "66deea4bc3e0a490eda8e08f4392dead3f62c177", "tag": "v0.0.1",
          "tag_object": "bd1d904ef84f6f0bc6f2b361de9bc6931bc74fa0"}
TOKEN_KEYS = {"CARGO_REGISTRY_TOKEN", "CARGO_REGISTRIES_CRATES_IO_TOKEN"}


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        assert key not in result, ("duplicate JSON field", key)
        result[key] = value
    return result


def load_plan(root):
    plan = json.loads((root / ".github/release-0.0.1.json").read_text(),
                      object_pairs_hook=unique_object)
    assert set(plan) == {"version", "tag", "published_source", "published",
                         "publish_order", "minimum_publish_interval_seconds"}
    assert plan["version"] == "0.0.1" and plan["tag"] == "faultline-rs-v0.0.1"
    assert plan["published_source"] == LEGACY, "unsupported original source"
    assert set(plan["published"]) == PUBLISHED
    assert all(re.fullmatch(r"[0-9a-f]{64}", value) for value in plan["published"].values())
    order = plan["publish_order"]
    assert len(order) == 9 and len(set(order)) == len(order)
    assert set(order) == PUBLIC - PUBLISHED, "unsupported publication selection"
    assert type(plan["minimum_publish_interval_seconds"]) is int
    assert plan["minimum_publish_interval_seconds"] == 60
    return plan


def git(root, *args):
    return subprocess.check_output(["git", *args], cwd=root, text=True).strip()


def git_file(root, source, path):
    return subprocess.check_output(["git", "show", source + ":" + path], cwd=root)


def internal_dependencies(manifest, include_dev=False):
    result = set()
    sections = [manifest] + list(manifest.get("target", {}).values())
    for section in sections:
        for kind in ["dependencies", "build-dependencies"] + (["dev-dependencies"] if include_dev else []):
            for alias, requirement in section.get(kind, {}).items():
                details = requirement if isinstance(requirement, dict) else {"version": requirement}
                name = details.get("package", alias)
                if not name.startswith("faultline-") or "version" not in details:
                    continue
                assert name in PUBLIC, ("unsupported internal dependency", name)
                assert details["version"] == "0.0.1", (name, details)
                result.add(name)
    return result


def internal_manifest_contract(manifest):
    result = {}
    sections = [("", manifest)] + list(manifest.get("target", {}).items())
    for target, section in sections:
        for kind in ["dependencies", "build-dependencies", "dev-dependencies"]:
            for alias, requirement in section.get(kind, {}).items():
                details = requirement if isinstance(requirement, dict) else {"version": requirement}
                name = details.get("package", alias)
                if not name.startswith("faultline-"):
                    continue
                if "version" not in details:
                    assert kind == "dev-dependencies" and name in PRIVATE, ("unversioned public dependency", name)
                    continue
                assert name in PUBLIC and details["version"] == "0.0.1"
                result[(target, kind, alias)] = {key: value for key, value in details.items() if key != "path"}
    return result


def workspace_packages(root, plan):
    workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]
    assert workspace["package"]["version"] == plan["version"]
    packages = {}
    for member in workspace["members"]:
        assert PurePosixPath(member).parts[:1] == ("crates",) and ".." not in PurePosixPath(member).parts
        manifest = tomllib.loads((root / member / "Cargo.toml").read_text())
        info = manifest["package"]
        name = info["name"]
        assert name not in packages, ("duplicate workspace package", name)
        assert info["version"] in ({"workspace": True}, plan["version"])
        if name in PRIVATE:
            assert info.get("publish") is False, name
        else:
            assert name in PUBLIC and info.get("publish", True) in (True, ["crates-io"]), name
        packages[name] = {"path": member, "manifest": manifest}
    assert set(packages) == PUBLIC | PRIVATE
    positions = {name: index for index, name in enumerate(plan["publish_order"])}
    for name, position in positions.items():
        for dependency in internal_dependencies(packages[name]["manifest"], True):
            assert dependency in PUBLISHED or positions[dependency] < position, ("publication dependency order", name, dependency)
    return packages


def validate_selection(root, environment):
    plan = load_plan(root)
    workspace_packages(root, plan)
    assert environment["GITHUB_EVENT_NAME"] == "push", "obsolete recovery dispatch is disabled"
    source = git(root, "rev-parse", "HEAD")
    assert source == environment["GITHUB_SHA"] == environment["GITHUB_WORKFLOW_SHA"]
    assert not git(root, "status", "--porcelain", "--untracked-files=no")
    legacy_ref = "refs/tags/" + LEGACY["tag"]
    assert git(root, "cat-file", "-t", legacy_ref) == "tag"
    assert git(root, "rev-parse", legacy_ref) == LEGACY["tag_object"]
    assert git(root, "rev-parse", legacy_ref + "^{commit}") == LEGACY["sha"]
    assert git(root, "rev-parse", legacy_ref + "^{tree}") == LEGACY["tree"]
    publish = environment["GITHUB_REF_TYPE"] == "tag"
    if publish:
        ref = environment["GITHUB_REF"]
        assert ref == "refs/tags/" + plan["tag"], "only the corrected release tag may publish"
        assert git(root, "cat-file", "-t", ref) == "tag", "release tags must be annotated"
        tag_object = git(root, "rev-parse", ref)
        assert git(root, "rev-parse", ref + "^{commit}") == source
        assert git(root, "rev-parse", ref + "^{tree}") == git(root, "rev-parse", "HEAD^{tree}")
        remote = git(root, "ls-remote", "origin", ref).split()
        assert remote == [tag_object, ref], "remote annotated tag differs"
        subprocess.run(["git", "merge-base", "--is-ancestor", "HEAD", "origin/main"], cwd=root, check=True)
    else:
        assert environment["GITHUB_REF_TYPE"] == "branch"
        assert re.fullmatch(r"refs/heads/(main|release/.+)", environment["GITHUB_REF"])
    return {"source_sha": source, "publish": str(publish).lower()}


def isolate_packages(root):
    target = root / "target"
    package = target / "package"
    if package.exists() or package.is_symlink():
        assert target.is_dir() and not target.is_symlink()
        assert package.is_dir() and not package.is_symlink()
        assert package.stat().st_uid == os.geteuid()
        # Cargo 1.93's uplift does not truncate old destinations. Preserve cache.
        quarantine = Path(tempfile.mkdtemp(prefix="package-quarantine-", dir=target)) / "package"
        package.rename(quarantine)
        print("Preserved cached package artifacts at", quarantine)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def single_gzip(data):
    stream = zlib.decompressobj(wbits=31)
    stream.decompress(data)
    stream.flush()
    assert stream.eof and not stream.unused_data, "incomplete gzip or trailing archive bytes"


def open_url(request):
    try:
        return urllib.request.urlopen(request, timeout=30)
    except urllib.error.HTTPError as error:
        if error.code == 429:
            raise SystemExit("Registry rate limited; Retry-After=" + str(error.headers.get("Retry-After", "unspecified")) + "; stopping without another upload") from None
        raise


def registry_version(name, version):
    request = urllib.request.Request(f"https://crates.io/api/v1/crates/{name}/{version}",
                                     headers={"User-Agent": "faultline-rs-release (" + REPOSITORY + ")"})
    try:
        with open_url(request) as response:
            result = json.load(response)["version"]
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise
    assert result["crate"] == name and result["num"] == version
    assert result["repository"] == REPOSITORY and not result["yanked"], name
    return result


def fetch_published(root):
    plan = load_plan(root)
    target = root / "target"
    assert not target.is_symlink()
    target.mkdir(exist_ok=True)
    directory = root / "target/published-anchors"
    directory.mkdir(parents=True, exist_ok=True)
    assert not directory.is_symlink()
    for name, checksum in plan["published"].items():
        published = registry_version(name, plan["version"])
        assert published and published["checksum"] == checksum, ("original registry identity mismatch", name)
        archive = directory / f"{name}-{plan['version']}.crate"
        assert not archive.is_symlink()
        if not archive.is_file() or sha256(archive.read_bytes()) != checksum:
            request = urllib.request.Request(f"https://static.crates.io/crates/{name}/{name}-{plan['version']}.crate",
                                             headers={"User-Agent": "faultline-rs-release (" + REPOSITORY + ")"})
            with open_url(request) as response:
                data = response.read(16 * 1024 * 1024 + 1)
            assert len(data) <= 16 * 1024 * 1024 and sha256(data) == checksum, name
            single_gzip(data)
            if archive.exists():
                assert archive.is_file() and archive.stat().st_uid == os.geteuid()
                quarantine = Path(tempfile.mkdtemp(prefix="published-anchor-quarantine-", dir=target))
                archive.rename(quarantine / archive.name)
                print("Preserved mismatching cached anchor at", quarantine / archive.name)
            archive.write_bytes(data)
        time.sleep(1)


def inspect_archive(root, archive, name, source, member, version, expected_members=None):
    data = archive.read_bytes()
    single_gzip(data)
    prefix = f"{name}-{version}/"
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as package:
        members = package.getmembers()
        names = [entry.name for entry in members]
        assert len(names) == len(set(names)), ("duplicate tar member", name)
        for entry in members:
            assert entry.isfile() and entry.name.startswith(prefix)
            assert ".." not in PurePosixPath(entry.name).parts
        files = {entry.name[len(prefix):]: package.extractfile(entry).read() for entry in members}
    if expected_members is not None:
        assert set(files) == set(expected_members) and len(expected_members) == len(set(expected_members)), ("archive member inventory mismatch", name)
    manifest = tomllib.loads(files["Cargo.toml"].decode())
    info = manifest["package"]
    assert info["name"] == name and info["version"] == version
    assert info["license"] == "Apache-2.0" and info["rust-version"] == "1.93"
    assert info["repository"] == REPOSITORY
    if name == "faultline-rs":
        binaries = manifest.get("bin", [])
        assert len(binaries) == 1 and binaries[0]["name"] == "faultline-rs" and binaries[0]["path"] == "src/main.rs", "normalized CLI binary identity mismatch"
    original = git_file(root, source, member + "/Cargo.toml")
    assert files["Cargo.toml.orig"] == original, ("original manifest source mismatch", name)
    assert internal_manifest_contract(manifest) == internal_manifest_contract(tomllib.loads(original.decode())), ("normalized internal manifest differs from source", name)
    assert files["README.md"] == git_file(root, source, "README.md"), ("README provenance mismatch", name)
    assert files["LICENSE"] == git_file(root, source, "LICENSE")
    assert b"END OF TERMS AND CONDITIONS" in files["LICENSE"]
    vcs = json.loads(files[".cargo_vcs_info.json"])
    assert vcs["git"]["sha1"] == source and not vcs["git"].get("dirty", False)
    assert vcs["path_in_vcs"] == member
    for path, contents in files.items():
        if path not in {"Cargo.toml", "Cargo.toml.orig", "Cargo.lock", ".cargo_vcs_info.json", "README.md", "LICENSE"}:
            assert contents == git_file(root, source, member + "/" + path), ("included source mismatch", name, path)
    internal_dependencies(manifest, True)
    lock = tomllib.loads(files["Cargo.lock"].decode())["package"]
    return {"name": name, "version": version, "sha256": sha256(data), "source_sha": source,
            "license_bytes": len(files["LICENSE"]), "readme_bytes": len(files["README.md"]),
            "description": info["description"], "files": sorted(files)}, manifest, lock


def inspect_archives(root, source):
    plan = load_plan(root)
    packages = workspace_packages(root, plan)
    fresh = set(plan["publish_order"])
    version = plan["version"]
    canonical = root / "target/package"
    anchors = root / "target/published-anchors"
    inventory = json.loads((canonical / "package-file-lists.json").read_text(), object_pairs_hook=unique_object)
    assert set(inventory) == fresh
    assert {p.name for p in canonical.glob("*.crate")} == {f"{name}-{version}.crate" for name in fresh}
    assert {p.name for p in anchors.glob("*.crate")} == {f"{name}-{version}.crate" for name in PUBLISHED}
    receipts, manifests, locks = {}, {}, {}
    for name in sorted(PUBLIC):
        original = name in PUBLISHED
        archive = (anchors if original else canonical) / f"{name}-{version}.crate"
        expected_source = LEGACY["sha"] if original else source
        receipt, manifest, lock = inspect_archive(root, archive, name, expected_source, packages[name]["path"], version,
                                                 None if original else inventory[name])
        if original:
            assert receipt["sha256"] == plan["published"][name], ("original archive hash mismatch", name)
            assert internal_dependencies(manifest, True) <= PUBLISHED, ("original anchor depends on corrected source", name)
        else:
            temporary = canonical / "tmp-crate" / archive.name
            assert temporary.read_bytes() == archive.read_bytes(), ("dry-run/canonical archive mismatch", name)
        receipts[name], manifests[name], locks[name] = receipt, manifest, lock
    edge_count = 0
    for name, records in locks.items():
        identities = [(p["name"], p["version"], p.get("source")) for p in records]
        assert len(identities) == len(set(identities)), ("duplicate lock package identity", name)
        assert all(p["name"] not in PRIVATE and (not p["name"].startswith("faultline-") or p["name"] in PUBLIC) for p in records), ("unsupported internal lock package", name)
        internal = [p for p in records if p["name"] in PUBLIC]
        assert len({p["name"] for p in internal}) == len(internal), ("duplicate internal lock name", name)
        own = [p for p in internal if p["name"] == name]
        assert len(own) == 1 and own[0]["version"] == version and "source" not in own[0] and "checksum" not in own[0]
        actual = {p["name"]: p for p in internal if p["name"] != name}
        expected = internal_dependencies(manifests[name], True)
        pending = list(expected)
        while pending:
            dependency = pending.pop()
            for child in internal_dependencies(manifests[dependency]):
                if child != name and child not in expected:
                    expected.add(child)
                    pending.append(child)
        assert set(actual) == expected, ("internal lock closure mismatch", name, set(actual), expected)
        by_name = {record["name"]: record for record in internal}
        for record in internal:
            direct_names = []
            for dependency in record.get("dependencies", []):
                if not dependency.split()[0].startswith("faultline-"):
                    continue
                match = re.fullmatch(r"(faultline-[A-Za-z0-9_-]+)(?: ([^ ()]+)(?: \(([^()]+)\))?)?", dependency)
                assert match and match[1] in by_name, ("unresolved internal lock edge", name, dependency)
                resolved = by_name[match[1]]
                assert match[2] is None or match[2] == resolved["version"], ("internal lock edge version mismatch", name, dependency)
                assert match[3] is None or match[3] == resolved.get("source"), ("internal lock edge source mismatch", name, dependency)
                direct_names.append(match[1])
            direct = set(direct_names)
            assert len(direct) == len(direct_names), ("duplicate internal lock edge", name, record["name"])
            assert direct == internal_dependencies(manifests[record["name"]], record["name"] == name), ("internal lock edge mismatch", name, record["name"])
        for dependency, record in actual.items():
            assert record["version"] == version and record["source"] == REGISTRY, (name, dependency)
            assert record["checksum"] == receipts[dependency]["sha256"], ("internal archive checksum mismatch", name, dependency)
        edge_count += len(actual)
    assert source == git(root, "rev-parse", "HEAD")
    result = {"source_sha": source, "source_tree": git(root, "rev-parse", "HEAD^{tree}"),
              "controller_sha": source, "published_source": plan["published_source"],
              "version": version, "tag": plan["tag"],
              "packages": [receipts[name] for name in plan["publish_order"]],
              "published_anchors": [receipts[name] for name in sorted(PUBLISHED)],
              "internal_lock_edges": edge_count}
    (canonical / "release-receipts.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    return result


def read_receipts(root):
    plan = load_plan(root)
    receipt = json.loads((root / "target/package/release-receipts.json").read_text(), object_pairs_hook=unique_object)
    assert receipt["controller_sha"] == receipt["source_sha"]
    assert receipt["published_source"] == plan["published_source"]
    assert receipt["source_sha"] == git(root, "rev-parse", "HEAD")
    assert receipt["source_tree"] == git(root, "rev-parse", "HEAD^{tree}")
    assert receipt["version"] == plan["version"] and receipt["tag"] == plan["tag"]
    assert [p["name"] for p in receipt["packages"]] == plan["publish_order"]
    assert [p["name"] for p in receipt["published_anchors"]] == sorted(PUBLISHED)
    for item in receipt["packages"] + receipt["published_anchors"]:
        original = item["name"] in PUBLISHED
        assert item["version"] == plan["version"]
        assert item["source_sha"] == (LEGACY["sha"] if original else receipt["source_sha"])
        archive = root / ("target/published-anchors" if original else "target/package") / f"{item['name']}-{item['version']}.crate"
        data = archive.read_bytes()
        single_gzip(data)
        assert sha256(data) == item["sha256"]
        if original:
            assert item["sha256"] == plan["published"][item["name"]]
    return receipt


def preflight(root, require_all=False):
    receipt = read_receipts(root)
    statuses = {}
    for item in receipt["published_anchors"] + receipt["packages"]:
        name = item["name"]
        published = registry_version(name, item["version"])
        if published is None:
            assert name not in PUBLISHED and not require_all, ("required registry version absent", name)
            statuses[name] = False
        else:
            assert published["checksum"] == item["sha256"], ("occupied registry checksum mismatch", name)
            statuses[name] = True
        print("Verified existing" if statuses[name] else "Verified absent", name, item["version"])
        time.sleep(1)
    return statuses


def public_environment():
    return {key: value for key, value in os.environ.items() if key not in TOKEN_KEYS}


def run_cargo(root, args, environment, token=None):
    if token is None:
        assert not TOKEN_KEYS & environment.keys(), "credentials reached a non-publishing Cargo process"
    result = subprocess.run(["cargo", *args], cwd=root, env=environment,
                            capture_output=True, text=True, check=False)
    output = result.stdout + result.stderr
    if token:
        output = output.replace(token, "[redacted]")
    print(output, end="")
    if result.returncode:
        raise SystemExit("Cargo failed; stopping without retrying an upload")
    return result.stdout


def qualify(root, dry_run=False):
    plan = load_plan(root)
    workspace_packages(root, plan)
    args = ["publish", "--dry-run"] if dry_run else ["package"]
    args += ["--registry", "crates-io", "--locked", "--jobs", "1"]
    for name in plan["publish_order"]:
        args += ["-p", name]
    run_cargo(root, args, public_environment())
    if not dry_run:
        inventory = {}
        for name in plan["publish_order"]:
            # Listing resolves the unchanged local workspace, not a registry
            # overlay. Offline mode cannot fetch missing registry metadata.
            files = run_cargo(root, ["package", "--list", "--offline", "--locked", "-p", name], public_environment()).splitlines()
            assert files and len(files) == len(set(files))
            assert {"Cargo.toml", "Cargo.toml.orig", "Cargo.lock", ".cargo_vcs_info.json"} <= set(files)
            assert all(not PurePosixPath(path).is_absolute() and ".." not in PurePosixPath(path).parts for path in files)
            inventory[name] = files
        (root / "target/package/package-file-lists.json").write_text(json.dumps(inventory, indent=2) + "\n")


def publish(root):
    # Remove the step-bound secret from inherited environments immediately.
    token = os.environ.pop("CARGO_REGISTRY_TOKEN", None)
    assert token and not os.environ.get("CARGO_REGISTRIES_CRATES_IO_TOKEN")
    assert validate_selection(root, os.environ)["publish"] == "true"
    plan = load_plan(root)
    receipt = read_receipts(root)
    preflight(root)
    public = public_environment()
    last_upload = None
    for item in receipt["packages"]:
        name, version = item["name"], item["version"]
        existing = registry_version(name, version)
        if existing is not None:
            assert existing["checksum"] == item["sha256"], ("occupied registry checksum mismatch", name)
            print("Already published with exact qualified checksum", name)
            continue
        if last_upload is not None:
            time.sleep(max(0, plan["minimum_publish_interval_seconds"] - (time.monotonic() - last_upload)))
        args = ["publish", "--no-verify", "--registry", "crates-io", "--locked", "--jobs", "1", "-p", name]
        # Prior dependencies are now present. Verify this exact single-package
        # publication archive without credentials before its upload process.
        run_cargo(root, args + ["--dry-run"], public)
        archive_name = f"{name}-{version}.crate"
        for directory in [root / "target/package", root / "target/package/tmp-crate"]:
            data = (directory / archive_name).read_bytes()
            single_gzip(data)
            assert sha256(data) == item["sha256"], ("publication archive differs from qualification", name)
        existing = registry_version(name, version)
        if existing is not None:
            assert existing["checksum"] == item["sha256"], ("concurrent occupied version mismatch", name)
            continue
        assert not git(root, "status", "--porcelain", "--untracked-files=no")
        run_cargo(root, args, dict(public, CARGO_REGISTRY_TOKEN=token), token=token)
        last_upload = time.monotonic()
        published = registry_version(name, version)
        assert published and published["checksum"] == item["sha256"], ("upload not yet confirmed", name)
        print("Confirmed qualified publication", name, version, item["sha256"])


def main():
    root = Path.cwd()
    command = sys.argv[1]
    if command != "publish":
        assert not any(os.environ.get(key) for key in TOKEN_KEYS), "publishing token reached a non-publishing step"
    if command == "select":
        result = validate_selection(root, os.environ)
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write("".join(f"{key}={value}\n" for key, value in result.items()))
        print("Validated corrected release selection", result)
    elif command == "isolate":
        isolate_packages(root)
    elif command == "anchors":
        fetch_published(root)
    elif command in {"package", "dry-run"}:
        qualify(root, command == "dry-run")
    elif command == "inspect":
        inspect_archives(root, os.environ["RELEASE_SOURCE_SHA"])
    elif command == "preflight":
        preflight(root)
    elif command == "publish":
        publish(root)
    elif command == "verify":
        preflight(root, require_all=True)
    else:
        raise SystemExit("Unsupported release command")


if __name__ == "__main__":
    main()
