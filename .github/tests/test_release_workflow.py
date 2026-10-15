"""Offline contracts against the real scoped release helper (Python 3.11+).

All .crate data is generated. Every subprocess and HTTP boundary is denied by
installation of the harness, then replaced with an explicit spy where needed.
No Cargo, Git subprocess, registry, credentials or environment management is performed.
"""
from __future__ import annotations

from contextlib import ExitStack, redirect_stdout
import copy
import gzip
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tarfile
import tomllib
import unittest
from unittest import mock
import urllib.error
import zlib

from release_fixtures import (ANCHORS, BUILD, DEV, FRESH, LICENSE, NEW_README,
                               NEW_SOURCE, NEW_TAG, NEW_TREE, NORMAL, OLD_README,
                               OLD_SOURCE, OLD_TAG, OLD_TREE, ORDER, PUBLIC,
                               REGISTRY, REPOSITORY, SyntheticRelease, VERSION,
                               archive_bytes, closure, sha, toml_lock)

HERE = Path(__file__).resolve()
DEFAULT_HELPER = HERE.parents[1] / "scripts/release.py"
if not DEFAULT_HELPER.exists():
    DEFAULT_HELPER = HERE.parents[2] / "proposed/.github/scripts/release.py"
HELPER = Path(os.environ.get("FAULTLINE_RELEASE_HELPER", DEFAULT_HELPER)).resolve()
SPEC = importlib.util.spec_from_file_location("faultline_release_under_test", HELPER)
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)
PUBLISH_ORDER = [name for name in ORDER if name in FRESH]
REAL_ANCHOR_HASHES = {
    "faultline-codes": "5264d08a500db06e13588eef398e3d89a93588d9e2b4ed08897ef8325b6d74cc",
    "faultline-types": "cad57d092855ffc41feaada58016c5c0d6b4ae62b09bbbf53c36f19ab6bb458a",
    "faultline-junit": "a27af02b740111d56bdc9a9b2d43b0afb0c661a3e110a532f4406fff97253de9",
    "faultline-loader": "dba9c88801b17ad020c625e7fbe9221feca13bd7f504c1bec2f3a5331dd43332",
    "faultline-localization": "86f3dcd7cb3f5ff44a1c1c3de1dc20123f2bdc884aa929a9a766d3ab5266e8b7",
}


class MixedReleaseContracts(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="faultline-mixed-contract-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.fixture = SyntheticRelease()
        self.fixture.write(self.root)
        self.source_files = copy.deepcopy(self.fixture.files)
        self.plan = {"version": VERSION, "tag": "faultline-rs-v0.0.1",
                     "published_source": copy.deepcopy(release.LEGACY),
                     "published": {name: sha(self.fixture.archives[name]) for name in sorted(ANCHORS)},
                     "publish_order": list(PUBLISH_ORDER),
                     "minimum_publish_interval_seconds": 60}
        self.save_plan()
        self.stage()
        self.boundaries = ExitStack()
        self.addCleanup(self.boundaries.close)
        self.boundaries.enter_context(mock.patch.object(release, "git", side_effect=self.git))
        self.boundaries.enter_context(mock.patch.object(release, "git_file", side_effect=self.git_file))
        self.boundaries.enter_context(mock.patch.object(release.subprocess, "run", side_effect=AssertionError("Unexpected subprocess")))
        self.boundaries.enter_context(mock.patch.object(release.subprocess, "check_output", side_effect=AssertionError("Unexpected subprocess")))
        self.boundaries.enter_context(mock.patch.object(release.urllib.request, "urlopen", side_effect=AssertionError("Unexpected HTTP request")))
        self.boundaries.enter_context(mock.patch.dict(os.environ, {}, clear=True))
        self.output = self.boundaries.enter_context(redirect_stdout(io.StringIO()))

    def save_plan(self):
        directory = self.root / ".github"
        directory.mkdir(exist_ok=True)
        (directory / "release-0.0.1.json").write_text(json.dumps(self.plan))

    def stage(self):
        package = self.root / "target/package"
        anchors = self.root / "target/published-anchors"
        anchors.mkdir(parents=True, exist_ok=True)
        temporary = package / "tmp-crate"
        temporary.mkdir(parents=True, exist_ok=True)
        (package / "release-receipts.json").unlink(missing_ok=True)
        (package / "package-file-lists.json").write_text(json.dumps({name: sorted(self.fixture.files[name]) for name in PUBLISH_ORDER}))
        for name, data in self.fixture.archives.items():
            canonical = (anchors if name in ANCHORS else package) / f"{name}-{VERSION}.crate"
            canonical.write_bytes(data)
            if name in ANCHORS:
                (package / canonical.name).unlink(missing_ok=True)
            else:
                (temporary / canonical.name).write_bytes(data)
        return package

    def replace(self, name, data, temporary=True):
        directory = "target/published-anchors" if name in ANCHORS else "target/package"
        path = self.root / directory / f"{name}-{VERSION}.crate"
        path.write_bytes(data)
        if temporary and name in FRESH:
            (self.root / "target/package/tmp-crate" / path.name).write_bytes(data)

    def mutated_lock(self, mutation, name="faultline-rs"):
        files = copy.deepcopy(self.fixture.files[name])
        records = copy.deepcopy(self.fixture.locks[name])
        mutation(records)
        files["Cargo.lock"] = toml_lock(records)
        self.replace(name, archive_bytes(name, files))

    def git(self, root, *args):
        self.assertEqual(Path(root), self.root)
        ref = "refs/tags/faultline-rs-v0.0.1"
        old = "refs/tags/v0.0.1"
        identities = {
            ("rev-parse", "HEAD"): NEW_SOURCE,
            ("rev-parse", "HEAD^{tree}"): NEW_TREE,
            ("status", "--porcelain", "--untracked-files=no"): "",
            ("cat-file", "-t", old): "tag", ("rev-parse", old): OLD_TAG,
            ("rev-parse", old + "^{commit}"): OLD_SOURCE,
            ("rev-parse", old + "^{tree}"): OLD_TREE,
            ("cat-file", "-t", ref): "tag", ("rev-parse", ref): NEW_TAG,
            ("rev-parse", ref + "^{commit}"): NEW_SOURCE,
            ("rev-parse", ref + "^{tree}"): NEW_TREE,
            ("ls-remote", "origin", ref): NEW_TAG + "\t" + ref,
        }
        self.assertIn(args, identities, "Unexpected Git read")
        return identities[args]

    def git_file(self, root, source, path):
        self.assertEqual(Path(root), self.root)
        self.assertIn(source, {OLD_SOURCE, NEW_SOURCE})
        if path == "README.md":
            return OLD_README if source == OLD_SOURCE else NEW_README
        if path == "LICENSE":
            return LICENSE
        prefix, relative = path.rsplit("/", 1) if "/src/" not in path else path.split("/src/", 1)
        if "/src/" in path:
            relative = "src/" + relative
        name = "faultline-rs" if prefix == "crates/faultline-cli" else prefix.removeprefix("crates/")
        self.assertIn(name, self.source_files)
        self.assertEqual(source == OLD_SOURCE, name in ANCHORS)
        return self.source_files[name]["Cargo.toml.orig" if relative == "Cargo.toml" else relative]

    def inspect(self):
        return release.inspect_archives(self.root, NEW_SOURCE)

    def expect_inspection_failure(self):
        receipt = self.root / "target/package/release-receipts.json"
        receipt.unlink(missing_ok=True)
        with self.assertRaises((AssertionError, KeyError, ValueError, OSError, EOFError)):
            self.inspect()
        self.assertFalse(receipt.exists(), "A failed qualification cannot emit receipts")

    def environment(self, **updates):
        result = {"GITHUB_EVENT_NAME": "push", "GITHUB_SHA": NEW_SOURCE,
                  "GITHUB_WORKFLOW_SHA": NEW_SOURCE, "GITHUB_REF_TYPE": "tag",
                  "GITHUB_REF": "refs/tags/faultline-rs-v0.0.1"}
        result.update(updates)
        return result

    def published(self, name, checksum=None):
        return {"crate": name, "num": VERSION, "repository": REPOSITORY,
                "yanked": False, "checksum": checksum or sha(self.fixture.archives[name])}

    def qualify_receipt(self):
        return self.inspect()

    def test_real_plan_keeps_nine_fresh_five_original_pins_and_distinct_tag(self):
        actual = release.load_plan(HELPER.parents[2])
        self.assertEqual(actual["published"], REAL_ANCHOR_HASHES)
        self.assertEqual(actual["published_source"], {"sha": OLD_SOURCE, "tree": OLD_TREE,
                                                       "tag": "v0.0.1", "tag_object": OLD_TAG})
        self.assertEqual(set(actual["publish_order"]), FRESH)
        self.assertEqual(len(actual["publish_order"]), 9)
        self.assertEqual(actual["version"], VERSION)
        self.assertEqual(actual["tag"], "faultline-rs-v0.0.1")

    def test_plan_rejects_wrong_inventory_duplicate_order_and_duplicate_json_fields(self):
        for mutate in [lambda p: p["publish_order"].append("faultline-fixtures"),
                       lambda p: p["publish_order"].__setitem__(0, p["publish_order"][1]),
                       lambda p: p["published"].pop("faultline-codes"),
                       lambda p: p.__setitem__("tag", "v0.0.1"),
                       lambda p: p["published_source"].__setitem__("sha", NEW_SOURCE),
                       lambda p: p.__setitem__("minimum_publish_interval_seconds", 0)]:
            with self.subTest(mutation=mutate):
                before = copy.deepcopy(self.plan)
                mutate(self.plan)
                self.save_plan()
                with self.assertRaises(AssertionError):
                    release.load_plan(self.root)
                self.plan = before
        self.save_plan()
        path = self.root / ".github/release-0.0.1.json"
        path.write_text(path.read_text().replace('"version": "0.0.1"', '"version": "0.0.1", "version": "0.0.1"'))
        with self.assertRaises(AssertionError):
            release.load_plan(self.root)

    def test_mixed_graph_receipts_keep_original_and_fresh_source_readme_and_hashes(self):
        result = self.inspect()
        self.assertEqual([item["name"] for item in result["packages"]], PUBLISH_ORDER)
        self.assertEqual({item["name"] for item in result["published_anchors"]}, ANCHORS)
        self.assertEqual(result["source_sha"], NEW_SOURCE)
        self.assertEqual(result["source_tree"], NEW_TREE)
        self.assertEqual(result["controller_sha"], NEW_SOURCE)
        self.assertEqual(result["published_source"], release.LEGACY)
        for item in result["packages"] + result["published_anchors"]:
            original = item["name"] in ANCHORS
            self.assertEqual(item["sha256"], sha(self.fixture.archives[item["name"]]))
            self.assertEqual(item["files"], sorted(self.fixture.files[item["name"]]))
            self.assertEqual(item["source_sha"], OLD_SOURCE if original else NEW_SOURCE)
            self.assertEqual(item["readme_bytes"], len(OLD_README if original else NEW_README))
        self.assertEqual(result["internal_lock_edges"], sum(len(closure(name)) for name in PUBLIC))
        self.assertIn("faultline-loader", closure("faultline-types"))
        self.assertNotIn("faultline-loader", closure("faultline-rs"))
        self.assertIn("faultline-junit", closure("faultline-app"))
        self.assertNotIn("faultline-junit", closure("faultline-rs"))
        self.assertEqual(release.read_receipts(self.root), result)

    def test_exact_fresh_and_anchor_archive_inventory_is_enforced(self):
        for directory, name, action in [("target/package", "faultline-rs", "missing"),
                                         ("target/published-anchors", "faultline-loader", "missing"),
                                         ("target/package", "faultline-codes", "extra"),
                                         ("target/published-anchors", "faultline-rs", "extra")]:
            with self.subTest(directory=directory, name=name, action=action):
                self.stage()
                path = self.root / directory / f"{name}-{VERSION}.crate"
                if action == "missing":
                    path.unlink()
                else:
                    path.write_bytes(self.fixture.archives[name])
                self.expect_inspection_failure()
                path.unlink(missing_ok=True)
        self.stage()

    def test_anchor_hash_source_readme_and_included_source_mutations_are_rejected(self):
        for name, path, data in [("faultline-codes", "README.md", NEW_README),
                                 ("faultline-rs", "README.md", OLD_README),
                                 ("faultline-rs", "src/main.rs", b"// Wrong source\n"),
                                 ("faultline-rs", "Cargo.toml.orig", b"# Rewritten manifest\n"),
                                 ("faultline-rs", ".cargo_vcs_info.json", json.dumps({"git": {"sha1": OLD_SOURCE}, "path_in_vcs": "crates/faultline-cli"}).encode())]:
            with self.subTest(name=name, path=path):
                self.stage()
                files = copy.deepcopy(self.fixture.files[name])
                files[path] = data
                self.replace(name, archive_bytes(name, files))
                self.expect_inspection_failure()
        self.stage()
        self.plan["published"]["faultline-codes"] = "0" * 64
        self.save_plan()
        self.expect_inspection_failure()

    def test_normalized_manifest_cannot_change_internal_source_dependencies(self):
        for mode in ("remove", "extra"):
            with self.subTest(mode=mode):
                self.stage()
                name = "faultline-rs"
                files = copy.deepcopy(self.fixture.files[name])
                records = copy.deepcopy(self.fixture.locks[name])
                manifest = files["Cargo.toml"].decode()
                if mode == "remove":
                    manifest = manifest.replace('faultline-app = "0.0.1"\n', "")
                    records = [record for record in records if record["name"] not in {"faultline-app", "faultline-surface"}]
                    records[0]["dependencies"].remove("faultline-app 0.0.1")
                else:
                    manifest = manifest.replace("[dependencies]\n", '[dependencies]\nfaultline-loader = "0.0.1"\n')
                    records[0]["dependencies"].append("faultline-loader 0.0.1")
                    records.append({"name": "faultline-loader", "version": VERSION, "source": REGISTRY,
                                    "checksum": sha(self.fixture.archives["faultline-loader"])})
                files["Cargo.toml"] = manifest.encode()
                files["Cargo.lock"] = toml_lock(records)
                self.replace(name, archive_bytes(name, files))
                self.expect_inspection_failure()

    def test_normalized_cli_binary_identity_is_exact(self):
        original = self.fixture.files["faultline-rs"]["Cargo.toml"]
        binary = b'\n[[bin]]\nname = "faultline-rs"\npath = "src/main.rs"\n'
        self.assertIn(binary, original)
        self.assertNotIn(b"[[bin]]", self.fixture.manifests["faultline-rs"])
        for replacement in (binary.replace(b'"faultline-rs"', b'"faultline-cli"'),
                            binary.replace(b'"src/main.rs"', b'"src/other.rs"'),
                            b"", binary + binary):
            with self.subTest(replacement=replacement):
                self.stage()
                files = copy.deepcopy(self.fixture.files["faultline-rs"])
                files["Cargo.toml"] = original.replace(binary, replacement)
                self.replace("faultline-rs", archive_bytes("faultline-rs", files))
                self.expect_inspection_failure()

    def test_orphan_obsolete_private_and_unsupported_internal_lock_records_are_rejected(self):
        for name in ("faultline-cli", "faultline-fixtures", "xtask", "faultline-unsupported"):
            with self.subTest(name=name):
                self.stage()
                self.mutated_lock(lambda records: records.append({"name": name, "version": VERSION}))
                self.expect_inspection_failure()

    def test_duplicate_tar_members_and_wrong_package_identity_are_rejected(self):
        name = "faultline-rs"
        stream = io.BytesIO()
        with tarfile.open(fileobj=stream, mode="w") as archive:
            members = list(self.fixture.files[name].items()) + [("README.md", NEW_README)]
            for path, data in members:
                member = tarfile.TarInfo(f"{name}-{VERSION}/{path}")
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))
        self.replace(name, gzip.compress(stream.getvalue(), mtime=0))
        self.expect_inspection_failure()
        self.stage()
        files = copy.deepcopy(self.fixture.files[name])
        files["Cargo.toml"] = files["Cargo.toml"].replace(b'name = "faultline-rs"', b'name = "faultline-app"', 1)
        self.replace(name, archive_bytes(name, files))
        self.expect_inspection_failure()

    def test_exact_source_inventory_rejects_missing_extra_duplicate_and_wrong_package_lists(self):
        inventory = self.root / "target/package/package-file-lists.json"
        for mode in ("missing-file", "extra-file", "duplicate-file", "missing-package", "extra-package", "duplicate-package", "missing-inventory"):
            with self.subTest(mode=mode):
                self.stage()
                lists = json.loads(inventory.read_text())
                if mode == "missing-file":
                    lists["faultline-rs"].remove("src/main.rs")
                elif mode == "extra-file":
                    lists["faultline-rs"].append("src/unexpected.rs")
                elif mode == "duplicate-file":
                    lists["faultline-rs"].append("src/main.rs")
                elif mode == "missing-package":
                    lists.pop("faultline-rs")
                elif mode == "extra-package":
                    lists["faultline-codes"] = sorted(self.fixture.files["faultline-codes"])
                if mode == "duplicate-package":
                    raw = json.dumps(lists)
                    inventory.write_text(raw[:-1] + ', "faultline-rs": ' + json.dumps(lists["faultline-rs"]) + "}")
                elif mode == "missing-inventory":
                    inventory.unlink()
                else:
                    inventory.write_text(json.dumps(lists))
                self.expect_inspection_failure()

    def test_source_inventory_rejects_missing_and_extra_archive_source_members(self):
        for mode in ("missing", "extra"):
            with self.subTest(mode=mode):
                self.stage()
                files = copy.deepcopy(self.fixture.files["faultline-rs"])
                if mode == "missing":
                    files.pop("src/main.rs")
                else:
                    files["src/unexpected.rs"] = b"// Unexpected generated member\n"
                self.replace("faultline-rs", archive_bytes("faultline-rs", files))
                self.expect_inspection_failure()

    def test_dry_run_must_reproduce_canonical_bytes_exactly(self):
        path = self.root / "target/package/tmp-crate/faultline-rs-0.0.1.crate"
        path.write_bytes(path.read_bytes() + b"different dry-run bytes")
        self.expect_inspection_failure()

    def test_complete_single_gzip_member_negative_controls(self):
        original = self.fixture.archives["faultline-rs"]
        crc = bytearray(original)
        crc[-8] ^= 1
        size = bytearray(original)
        size[-4] ^= 1
        controls = {"nonzero-tail": original + b"stale", "zero-tail": original + b"\0",
                    "second-member": original + gzip.compress(b"second", mtime=0),
                    "truncated-trailer": original[:-1], "truncated-body": original[:len(original)//2],
                    "wrong-crc": bytes(crc), "wrong-size": bytes(size)}
        for label, data in controls.items():
            with self.subTest(control=label):
                self.stage()
                self.replace("faultline-rs", data)
                with self.assertRaises((AssertionError, zlib.error)):
                    release.single_gzip(data)
                with self.assertRaises((AssertionError, zlib.error)):
                    self.inspect()
                self.assertFalse((self.root / "target/package/release-receipts.json").exists())

    def test_cache_nontruncating_uplift_red_then_recoverable_isolation_green(self):
        package = self.root / "target/package"
        original = self.fixture.archives["faultline-rs"]
        canonical = package / "faultline-rs-0.0.1.crate"
        canonical.write_bytes(b"x" * (len(original) + 23))
        temporary = package / "tmp-crate" / canonical.name
        # Match Cargo's nontruncating uplift instead of just concatenating bytes.
        fd = os.open(canonical, os.O_CREAT | os.O_RDWR, 0o600)
        with os.fdopen(fd, "r+b") as output, temporary.open("rb") as source:
            shutil.copyfileobj(source, output)
        self.assertGreater(canonical.stat().st_size, len(original))
        with tarfile.open(canonical, "r:gz") as archive:
            self.assertTrue(archive.getnames(), "Old first-member-only inspection accepts this")
        self.expect_inspection_failure()
        before = {path.relative_to(package): path.read_bytes() for path in package.rglob("*") if path.is_file()}
        inode = package.stat().st_ino
        marker = self.root / "target/debug/build-cache-marker"
        marker.parent.mkdir()
        marker.write_bytes(b"Keep unrelated build cache")
        release.isolate_packages(self.root)
        self.assertFalse(package.exists())
        preserved, = (self.root / "target").glob("package-quarantine-*/package")
        self.assertEqual(preserved.stat().st_ino, inode)
        self.assertEqual(before, {path.relative_to(preserved): path.read_bytes() for path in preserved.rglob("*") if path.is_file()})
        self.assertEqual(marker.read_bytes(), b"Keep unrelated build cache")
        self.stage()
        self.inspect()
        self.assertEqual(before, {path.relative_to(preserved): path.read_bytes() for path in preserved.rglob("*") if path.is_file()})
        shutil.rmtree(package)
        preserved.rename(package)
        self.assertEqual(before, {path.relative_to(package): path.read_bytes() for path in package.rglob("*") if path.is_file()})

    def test_cold_cache_noop_and_symlink_cache_rejection_preserve_target(self):
        other = self.root / "empty"
        other.mkdir()
        release.isolate_packages(other)
        self.assertFalse((other / "target").exists())
        target = other / "target"
        target.mkdir()
        elsewhere = other / "untouched"
        elsewhere.mkdir()
        (elsewhere / "marker").write_bytes(b"untouched")
        (target / "package").symlink_to(elsewhere, target_is_directory=True)
        with self.assertRaises(AssertionError):
            release.isolate_packages(other)
        self.assertTrue((target / "package").is_symlink())
        self.assertEqual((elsewhere / "marker").read_bytes(), b"untouched")

    def test_full_internal_closure_rejects_missing_extra_and_wrong_records(self):
        for mutate in [lambda records: records.pop(1),
                       lambda records: records.append({"name": "faultline-loader", "version": VERSION, "source": REGISTRY, "checksum": sha(self.fixture.archives["faultline-loader"])}),
                       lambda records: records[1].__setitem__("checksum", "0" * 64),
                       lambda records: records[1].__setitem__("version", "0.0.2"),
                       lambda records: records[1].__setitem__("source", "registry+https://example.invalid/index")]:
            with self.subTest(mutation=mutate):
                self.stage()
                self.mutated_lock(mutate)
                self.expect_inspection_failure()

    def test_duplicate_lock_identities_are_rejected(self):
        self.mutated_lock(lambda records: records.append(copy.deepcopy(records[1])))
        self.expect_inspection_failure()

    def test_duplicate_internal_name_with_different_identity_is_rejected(self):
        def mutate(records):
            extra = copy.deepcopy(records[1])
            extra["version"] = "0.0.2"
            records.insert(1, extra)  # Valid record comes later; dict collapse is unsafe.
        self.mutated_lock(mutate)
        self.expect_inspection_failure()

    def test_lock_dependency_edges_reject_wrong_missing_and_extra_edges(self):
        for mode in ("wrong-version", "wrong-source", "missing", "extra", "duplicate"):
            with self.subTest(mode=mode):
                self.stage()
                def mutate(records):
                    edges = records[0]["dependencies"]
                    if mode == "wrong-version":
                        edges[0] = "faultline-app 0.0.2"
                    elif mode == "wrong-source":
                        edges[0] = "faultline-app 0.0.1 (registry+https://example.invalid/index)"
                    elif mode == "duplicate":
                        edges.append(edges[0])
                    elif mode == "missing":
                        edges.pop()
                    else:
                        edges.append("faultline-loader 0.0.1")
                self.mutated_lock(mutate)
                self.expect_inspection_failure()

    def test_missing_root_and_replaced_root_lock_identities_are_rejected(self):
        for mode in ("missing", "wrong-version", "registry-root"):
            with self.subTest(mode=mode):
                self.stage()
                def mutate(records):
                    if mode == "missing":
                        records.pop(0)
                    elif mode == "wrong-version":
                        records[0]["version"] = "0.0.2"
                    else:
                        records[0]["source"] = REGISTRY
                        records[0]["checksum"] = "0" * 64
                self.mutated_lock(mutate)
                self.expect_inspection_failure()

    def test_root_dev_edge_is_required_but_transitive_dev_edge_is_forbidden(self):
        self.mutated_lock(lambda records: records.__setitem__(slice(None), [record for record in records if record["name"] != "faultline-junit"]), name="faultline-app")
        self.expect_inspection_failure()
        self.stage()
        def include_transitive_dev(records):
            records.append({"name": "faultline-loader", "version": VERSION, "source": REGISTRY,
                            "checksum": sha(self.fixture.archives["faultline-loader"])})
        self.mutated_lock(include_transitive_dev)
        self.expect_inspection_failure()

    def test_target_alias_build_and_root_dev_dependency_rules(self):
        manifest = {"dependencies": {"codes_alias": {"package": "faultline-codes", "version": VERSION}, "private_fixture": {"package": "faultline-fixtures", "path": "../faultline-fixtures"}},
                    "build-dependencies": {"faultline-types": VERSION},
                    "dev-dependencies": {"faultline-junit": VERSION},
                    "target": {"cfg(unix)": {"dependencies": {"faultline-ports": VERSION}, "dev-dependencies": {"faultline-loader": VERSION}}}}
        self.assertEqual(release.internal_dependencies(manifest), {"faultline-codes", "faultline-types", "faultline-ports"})
        self.assertEqual(release.internal_dependencies(manifest, True), {"faultline-codes", "faultline-types", "faultline-ports", "faultline-junit", "faultline-loader"})
        manifest["dependencies"]["faultline-app"] = "0.0.2"
        with self.assertRaises(AssertionError):
            release.internal_dependencies(manifest, True)

    def test_new_annotated_tag_only_and_branch_is_check_only(self):
        calls = []
        def git_command(command, **kwargs):
            calls.append(command)
            self.assertEqual(command, ["git", "merge-base", "--is-ancestor", "HEAD", "origin/main"])
            return subprocess.CompletedProcess(command, 0)
        with mock.patch.object(release.subprocess, "run", side_effect=git_command):
            self.assertEqual(release.validate_selection(self.root, self.environment()), {"source_sha": NEW_SOURCE, "publish": "true"})
        self.assertEqual(len(calls), 1)
        self.assertEqual(release.validate_selection(self.root, self.environment(GITHUB_REF_TYPE="branch", GITHUB_REF="refs/heads/main"))["publish"], "false")
        for changes in [{"GITHUB_EVENT_NAME": "workflow_dispatch"}, {"GITHUB_REF": "refs/tags/v0.0.1"}, {"GITHUB_SHA": OLD_SOURCE}, {"GITHUB_WORKFLOW_SHA": OLD_SOURCE}, {"GITHUB_REF_TYPE": "branch", "GITHUB_REF": "refs/heads/unrelated"}]:
            with self.subTest(changes=changes), self.assertRaises(AssertionError):
                release.validate_selection(self.root, self.environment(**changes))

    def test_annotated_checkout_fallback_red_and_explicit_sha_green_preserve_remote(self):
        ref = "refs/tags/faultline-rs-v0.0.1"
        # Model the exact checkout fallback: raw annotated tag object A differs
        # from event commit C; a fallback fetch C:refs/tags/name rewrites only
        # the runner-local ref to C. Remote A must remain unchanged.
        local = {"object": NEW_TAG, "kind": "tag"}
        remote = NEW_TAG
        def git(root, *args):
            if args == ("rev-parse", ref):
                return local["object"]
            if args == ("cat-file", "-t", ref):
                return local["kind"]
            if args == ("ls-remote", "origin", ref):
                return remote + "\t" + ref
            return self.git(root, *args)
        self.assertNotEqual(local["object"], NEW_SOURCE)
        local.update(object=NEW_SOURCE, kind="commit")
        with mock.patch.object(release, "git", side_effect=git), self.assertRaises(AssertionError):
            release.validate_selection(self.root, self.environment())
        self.assertEqual(remote, NEW_TAG)
        # SHA-only checkout gives the action no tag ref to force-rewrite.
        local.update(object=NEW_TAG, kind="tag")
        with mock.patch.object(release, "git", side_effect=git), mock.patch.object(release.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)):
            self.assertEqual(release.validate_selection(self.root, self.environment()), {"source_sha": NEW_SOURCE, "publish": "true"})
        self.assertEqual(local, {"object": NEW_TAG, "kind": "tag"})
        self.assertEqual(remote, NEW_TAG)

    def test_lightweight_moved_remote_dirty_and_legacy_mutations_fail_selection(self):
        changes = {("cat-file", "-t", "refs/tags/faultline-rs-v0.0.1"): "commit",
                   ("rev-parse", "refs/tags/faultline-rs-v0.0.1^{commit}"): OLD_SOURCE,
                   ("rev-parse", "refs/tags/faultline-rs-v0.0.1^{tree}"): OLD_TREE,
                   ("ls-remote", "origin", "refs/tags/faultline-rs-v0.0.1"): OLD_TAG + "\trefs/tags/faultline-rs-v0.0.1",
                   ("status", "--porcelain", "--untracked-files=no"): " M Cargo.toml",
                   ("rev-parse", "refs/tags/v0.0.1"): NEW_TAG,
                   ("rev-parse", "refs/tags/v0.0.1^{commit}"): NEW_SOURCE,
                   ("rev-parse", "refs/tags/v0.0.1^{tree}"): NEW_TREE}
        for args, wrong in changes.items():
            with self.subTest(args=args):
                def git(root, *actual):
                    return wrong if actual == args else self.git(root, *actual)
                with mock.patch.object(release, "git", side_effect=git), self.assertRaises(AssertionError):
                    release.validate_selection(self.root, self.environment())

    def test_qualification_selects_exact_nine_with_no_token(self):
        calls = []
        def run(command, **kwargs):
            calls.append(command)
            self.assertFalse(release.TOKEN_KEYS & kwargs["env"].keys())
            if "--list" in command:
                self.assertEqual(command[:5], ["cargo", "package", "--list", "--offline", "--locked"])
                self.assertNotIn("--registry", command)
                self.assertNotIn("--jobs", command)
                name = command[command.index("-p") + 1]
                return subprocess.CompletedProcess(command, 0, "\n".join(sorted(self.fixture.files[name])) + "\n", "")
            return subprocess.CompletedProcess(command, 0, "", "")
        os.environ["CARGO_REGISTRY_TOKEN"] = "generated-sentinel-not-a-credential"
        with mock.patch.object(release.subprocess, "run", side_effect=run):
            release.qualify(self.root)
            release.qualify(self.root, dry_run=True)
        self.assertEqual(len(calls), 11)
        for command in (calls[0], calls[-1]):
            self.assertNotIn("--workspace", command)
            selected = [command[index + 1] for index, arg in enumerate(command) if arg == "-p"]
            self.assertEqual(selected, PUBLISH_ORDER)
            self.assertFalse(ANCHORS & set(selected))
        self.assertEqual(calls[0][1], "package")
        self.assertIn("--dry-run", calls[-1])
        self.assertEqual([command[-1] for command in calls[1:-1]], PUBLISH_ORDER)
        inventory = json.loads((self.root / "target/package/package-file-lists.json").read_text())
        self.assertEqual(inventory, {name: sorted(self.fixture.files[name]) for name in PUBLISH_ORDER})

    def test_offline_source_list_failure_stops_without_registry_fallback_or_partial_inventory(self):
        inventory = self.root / "target/package/package-file-lists.json"
        inventory.unlink()
        calls = []
        failing = PUBLISH_ORDER[2]
        def run(command, **kwargs):
            calls.append(command)
            self.assertFalse(release.TOKEN_KEYS & kwargs["env"].keys())
            if "--list" not in command:
                self.assertEqual(command[1], "package")
                return subprocess.CompletedProcess(command, 0, "", "")
            self.assertEqual(command, ["cargo", "package", "--list", "--offline", "--locked", "-p", command[-1]])
            if command[-1] == failing:
                return subprocess.CompletedProcess(command, 1, "", "offline metadata unavailable")
            return subprocess.CompletedProcess(command, 0, "\n".join(sorted(self.fixture.files[command[-1]])), "")
        with mock.patch.object(release.subprocess, "run", side_effect=run), self.assertRaises(SystemExit):
            release.qualify(self.root)
        self.assertEqual(len(calls), 4)
        self.assertEqual([command[-1] for command in calls[1:]], PUBLISH_ORDER[:3])
        self.assertFalse(inventory.exists())

    def test_offline_source_list_rejects_empty_duplicate_missing_required_and_unsafe_paths(self):
        inventory = self.root / "target/package/package-file-lists.json"
        expected = sorted(self.fixture.files[PUBLISH_ORDER[0]])
        for label, files in [("empty", []), ("duplicate", expected + [expected[0]]),
                             ("missing-required", [path for path in expected if path != "Cargo.lock"]),
                             ("absolute", expected + ["/unexpected"]),
                             ("traversal", expected + ["../unexpected"])]:
            with self.subTest(control=label):
                inventory.unlink(missing_ok=True)
                calls = []
                def run(command, **kwargs):
                    calls.append(command)
                    return subprocess.CompletedProcess(command, 0, "\n".join(files) if "--list" in command else "", "")
                with mock.patch.object(release.subprocess, "run", side_effect=run), self.assertRaises(AssertionError):
                    release.qualify(self.root)
                self.assertEqual(len(calls), 2)
                self.assertFalse(inventory.exists())

    def test_nonpublish_main_rejects_tokens_and_public_environment_strips_both_keys(self):
        for key in release.TOKEN_KEYS:
            with self.subTest(key=key), mock.patch.dict(os.environ, {key: "generated-sentinel"}, clear=True), mock.patch.object(sys, "argv", [str(HELPER), "inspect"]):
                with self.assertRaises(AssertionError):
                    release.main()
                self.assertNotIn(key, release.public_environment())

    def publication_harness(self, initially_published=(), fail_upload=None, mismatch=None, mutate_dry_run=None, unconfirmed=None):
        self.qualify_receipt()
        state = set(ANCHORS) | set(initially_published)
        events = []
        clock = [0.0]
        sentinel = "generated-token-sentinel-not-a-real-credential"
        def sleep(seconds):
            self.assertGreaterEqual(seconds, 0)
            events.append(("sleep", seconds))
            clock[0] += seconds
        def registry(name, version):
            self.assertEqual(version, VERSION)
            self.assertFalse(release.TOKEN_KEYS & os.environ.keys())
            events.append(("registry", name))
            if mismatch and name == mismatch:
                return self.published(name, "0" * 64)
            if name == unconfirmed and name in state:
                return None
            return self.published(name) if name in state else None
        def cargo(command, **kwargs):
            self.assertEqual(command[:2], ["cargo", "publish"])
            self.assertIn("--no-verify", command)
            self.assertIn("--locked", command)
            self.assertNotIn("--workspace", command)
            name = command[command.index("-p") + 1]
            self.assertIn(name, FRESH)
            dry = "--dry-run" in command
            events.append(("dry" if dry else "upload", name, clock[0]))
            keys = release.TOKEN_KEYS & kwargs["env"].keys()
            self.assertEqual(keys, set() if dry else {"CARGO_REGISTRY_TOKEN"})
            if not dry:
                self.assertEqual(kwargs["env"]["CARGO_REGISTRY_TOKEN"], sentinel)
                if name == fail_upload:
                    return subprocess.CompletedProcess(command, 1, "", "429 Retry-After: 120 " + sentinel)
                state.add(name)
            elif name == mutate_dry_run:
                path = self.root / f"target/package/tmp-crate/{name}-{VERSION}.crate"
                path.write_bytes(path.read_bytes() + b"stale")
            return subprocess.CompletedProcess(command, 0, "", "")
        context = ExitStack()
        context.enter_context(mock.patch.dict(os.environ, dict(self.environment(), CARGO_REGISTRY_TOKEN=sentinel), clear=True))
        context.enter_context(mock.patch.object(release, "registry_version", side_effect=registry))
        context.enter_context(mock.patch.object(release.subprocess, "run", side_effect=cargo))
        context.enter_context(mock.patch.object(release.time, "sleep", side_effect=sleep))
        context.enter_context(mock.patch.object(release.time, "monotonic", side_effect=lambda: clock[0]))
        # Selection has its own exact-head tests; don't let Git run in upload tests.
        context.enter_context(mock.patch.object(release, "validate_selection", return_value={"source_sha": NEW_SOURCE, "publish": "true"}))
        return context, events, state, sentinel

    def test_paced_ordered_single_crate_dry_run_exact_bytes_upload_and_confirmation(self):
        context, events, state, sentinel = self.publication_harness()
        with context:
            release.publish(self.root)
        uploads = [event for event in events if event[0] == "upload"]
        self.assertEqual([event[1] for event in uploads], PUBLISH_ORDER)
        self.assertEqual(state, PUBLIC)
        for earlier, later in zip(uploads, uploads[1:]):
            self.assertGreaterEqual(later[2] - earlier[2], 60)
        for name in PUBLISH_ORDER:
            dry = next(index for index, event in enumerate(events) if event[:2] == ("dry", name))
            upload = next(index for index, event in enumerate(events) if event[:2] == ("upload", name))
            self.assertLess(dry, upload)
            self.assertEqual(events[upload - 1][:2], ("registry", name))
            self.assertEqual(events[upload + 1][:2], ("registry", name))
        self.assertNotIn(sentinel, self.output.getvalue())

    def test_partial_success_stops_then_resume_skips_matching_published_versions(self):
        failed = PUBLISH_ORDER[2]
        context, events, state, sentinel = self.publication_harness(fail_upload=failed)
        with context, self.assertRaises(SystemExit):
            release.publish(self.root)
        self.assertEqual([event[1] for event in events if event[0] == "upload"], PUBLISH_ORDER[:3])
        self.assertEqual(state - ANCHORS, set(PUBLISH_ORDER[:2]))
        self.assertIn("Retry-After: 120", self.output.getvalue())
        self.assertNotIn(sentinel, self.output.getvalue())
        context, events, state, _ = self.publication_harness(initially_published=state - ANCHORS)
        with context:
            release.publish(self.root)
        self.assertEqual([event[1] for event in events if event[0] == "upload"], PUBLISH_ORDER[2:])
        self.assertEqual([event[1] for event in events if event[0] == "dry"], PUBLISH_ORDER[2:])
        self.assertEqual(state, PUBLIC)

    def test_occupied_mismatch_stops_before_any_dry_run_or_upload(self):
        for name in (PUBLISH_ORDER[3], "faultline-codes"):
            with self.subTest(name=name):
                context, events, _, _ = self.publication_harness(mismatch=name)
                with context, self.assertRaises(AssertionError):
                    release.publish(self.root)
                self.assertFalse([event for event in events if event[0] in {"dry", "upload"}])

    def test_unconfirmed_upload_stops_before_next_crate_without_retry(self):
        context, events, state, _ = self.publication_harness(unconfirmed=PUBLISH_ORDER[0])
        with context, self.assertRaises(AssertionError):
            release.publish(self.root)
        self.assertEqual([event[1] for event in events if event[0] == "upload"], PUBLISH_ORDER[:1])
        self.assertEqual(state - ANCHORS, set(PUBLISH_ORDER[:1]))

    def test_changed_single_crate_dry_run_bytes_stop_before_upload(self):
        context, events, _, _ = self.publication_harness(mutate_dry_run=PUBLISH_ORDER[0])
        with context, self.assertRaises((AssertionError, zlib.error)):
            release.publish(self.root)
        self.assertEqual(len([event for event in events if event[0] == "dry"]), 1)
        self.assertFalse([event for event in events if event[0] == "upload"])

    def test_anchor_fetch_verifies_hash_and_preserves_matching_cached_bytes(self):
        anchors = self.root / "target/published-anchors"
        for path in anchors.glob("*.crate"):
            path.unlink()
        downloaded = []
        def http(request, **kwargs):
            name = request.full_url.split("/crates/", 1)[1].split("/", 1)[0]
            self.assertIn(name, ANCHORS)
            downloaded.append(name)
            response = mock.MagicMock()
            response.__enter__.return_value = io.BytesIO(self.fixture.archives[name])
            return response
        with mock.patch.object(release, "registry_version", side_effect=lambda name, version: self.published(name)), mock.patch.object(release.urllib.request, "urlopen", side_effect=http), mock.patch.object(release.time, "sleep") as sleep:
            release.fetch_published(self.root)
            self.assertEqual(set(downloaded), ANCHORS)
            self.assertEqual(sleep.call_count, 5)
            for name in ANCHORS:
                self.assertEqual((anchors / f"{name}-{VERSION}.crate").read_bytes(), self.fixture.archives[name])
            downloaded.clear()
            release.fetch_published(self.root)
            self.assertEqual(downloaded, [], "Matching immutable anchors must not be rebuilt or redownloaded")
        for path in anchors.glob("*.crate"):
            path.unlink()
        response = mock.MagicMock()
        response.__enter__.return_value = io.BytesIO(b"Wrong generated archive bytes")
        with mock.patch.object(release, "registry_version", side_effect=lambda name, version: self.published(name)), mock.patch.object(release.urllib.request, "urlopen", return_value=response), mock.patch.object(release.time, "sleep"), self.assertRaises(AssertionError):
            release.fetch_published(self.root)
        self.assertEqual(list(anchors.glob("*.crate")), [])

    def test_mismatching_cached_anchor_is_quarantined_before_exact_replacement(self):
        name = "faultline-codes"
        archive = self.root / f"target/published-anchors/{name}-{VERSION}.crate"
        evidence = b"Owned stale anchor evidence"
        archive.write_bytes(evidence)
        response = mock.MagicMock()
        response.__enter__.return_value = io.BytesIO(self.fixture.archives[name])
        with mock.patch.object(release, "registry_version", side_effect=lambda package, version: self.published(package)), \
                mock.patch.object(release.urllib.request, "urlopen", return_value=response) as request, \
                mock.patch.object(release.time, "sleep"):
            release.fetch_published(self.root)
        self.assertEqual(request.call_count, 1)
        self.assertEqual(archive.read_bytes(), self.fixture.archives[name])
        preserved = list((self.root / "target").glob(f"published-anchor-quarantine-*/{archive.name}"))
        self.assertEqual(len(preserved), 1)
        self.assertEqual(preserved[0].read_bytes(), evidence)

    def test_registry_429_during_publication_preflight_has_no_upload_or_retry(self):
        context, events, _, _ = self.publication_harness()
        with context, mock.patch.object(release, "registry_version", side_effect=SystemExit("Registry rate limited; Retry-After=120; stopping")) as registry, self.assertRaisesRegex(SystemExit, "Retry-After=120"):
            release.publish(self.root)
        self.assertEqual(registry.call_count, 1)
        self.assertFalse([event for event in events if event[0] in {"dry", "upload"}])

    def test_registry_429_reports_retry_after_and_does_not_retry(self):
        error = urllib.error.HTTPError("https://crates.io/generated", 429, "Too Many Requests", {"Retry-After": "120"}, None)
        with mock.patch.object(release.urllib.request, "urlopen", side_effect=error) as request:
            with self.assertRaisesRegex(SystemExit, "Retry-After=120"):
                release.registry_version("faultline-rs", VERSION)
            self.assertEqual(request.call_count, 1)

    def test_registry_404_absence_and_wrong_identity_yanked_repository_fail_closed(self):
        error = urllib.error.HTTPError("https://crates.io/generated", 404, "Not Found", {}, None)
        with mock.patch.object(release.urllib.request, "urlopen", side_effect=error):
            self.assertIsNone(release.registry_version("faultline-rs", VERSION))
        for field, value in [("crate", "faultline-cli"), ("num", "0.0.2"), ("yanked", True), ("repository", "https://example.invalid")]:
            with self.subTest(field=field):
                data = self.published("faultline-rs")
                data[field] = value
                response = mock.MagicMock()
                response.__enter__.return_value = io.StringIO(json.dumps({"version": data}))
                with mock.patch.object(release.urllib.request, "urlopen", return_value=response), self.assertRaises(AssertionError):
                    release.registry_version("faultline-rs", VERSION)

    def test_normal_workflow_runs_contracts_then_isolation_qualification_and_gated_publish(self):
        workflow = (HELPER.parents[1] / "workflows/release.yml").read_text()
        self.assertIn("          ref: ${{ github.sha }}\n", workflow)
        self.assertIn("persist-credentials: false", workflow)
        self.assertNotIn("ref: refs/tags/v0.0.1", workflow)
        self.assertNotIn("workflow_dispatch:", workflow)
        self.assertIn('tags: ["faultline-rs-v0.0.1"]', workflow)
        self.assertIn("cancel-in-progress: false", workflow)
        self.assertIn("contents: read", workflow)
        contracts = min(index for index in [workflow.find("python3 .github/tests/test_release_workflow.py"),
                                              workflow.find("python3 -m unittest discover -s .github/tests")] if index >= 0)
        cache = workflow.index("uses: actions/cache@v4")
        isolation = workflow.index("python3 .github/scripts/release.py isolate")
        self.assertLess(contracts, isolation)
        self.assertLess(cache, isolation)
        previous = isolation
        for command in ("package", "dry-run", "inspect", "preflight", "publish", "verify"):
            current = workflow.index("python3 .github/scripts/release.py " + command)
            self.assertLess(previous, current)
            previous = current
        self.assertLess(workflow.index("python3 .github/scripts/release.py inspect"), workflow.index("uses: actions/upload-artifact@v4"))
        artifact = workflow.split("uses: actions/upload-artifact@v4", 1)[1].split("\n      - ", 1)[0]
        self.assertIn("target/package/package-file-lists.json", artifact)
        self.assertIn("target/package/release-receipts.json", artifact)
        self.assertIn("target/published-anchors/*.crate", artifact)
        self.assertEqual(workflow.count("${{ secrets.CARGO_REGISTRY_TOKEN }}"), 1)
        steps = workflow.split("\n      - ")
        token_steps = [step for step in steps if "${{ secrets.CARGO_REGISTRY_TOKEN }}" in step]
        self.assertEqual(len(token_steps), 1)
        self.assertIn("python3 .github/scripts/release.py publish", token_steps[0])
        self.assertIn("if: steps.selection.outputs.publish == 'true'", token_steps[0])
        self.assertNotIn("cargo publish --workspace", workflow)
        self.assertIn("cargo install faultline-rs", workflow)
        self.assertIn("bin/faultline-rs", workflow)
        self.assertNotIn("bin/faultline-cli", workflow)

    def test_all_mixed_contracts_are_cataloged_and_exact_files_have_policy_receipts(self):
        repository = HELPER.parents[2]
        scenarios = (repository / "docs/scenarios/scenario_index.md").read_text()
        for name in unittest.defaultTestLoader.getTestCaseNames(type(self)):
            self.assertTrue("| Python: `" + name + "` |" in scenarios, "Missing Python scenario receipt: " + name)
        policy = tomllib.loads((repository / "policy/non-rust-allowlist.toml").read_text())
        covered = {item.get("path", item.get("glob")): item for item in policy["allow"]}
        for path in (".github/scripts/release.py", ".github/release-0.0.1.json", ".github/tests/release_fixtures.py"):
            self.assertIn(path, covered)
            self.assertEqual(covered[path]["owner"], "release/ci")
            self.assertTrue(covered[path]["covered_by"])
        self.assertTrue(".github/tests/test_release_workflow.py" in covered or ".github/tests/test_mixed_release.py" in covered)

    def test_verification_requires_all_fourteen_qualified_registry_identities(self):
        self.qualify_receipt()
        with mock.patch.object(release, "registry_version", side_effect=lambda name, version: self.published(name)), mock.patch.object(release.time, "sleep"):
            self.assertTrue(all(release.preflight(self.root, require_all=True).values()))
        with mock.patch.object(release, "registry_version", side_effect=lambda name, version: None if name == "faultline-rs" else self.published(name)), mock.patch.object(release.time, "sleep"), self.assertRaises(AssertionError):
            release.preflight(self.root, require_all=True)


if __name__ == "__main__":
    unittest.main(verbosity=2)
