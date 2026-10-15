"""Offline contracts for the actual inline release guards (Python 3.11+).

Generated archives are fixtures, not Cargo qualification. No Cargo, network,
credentials, or downloaded release artifacts are used. To reproduce RED against
an older workflow, set FAULTLINE_RELEASE_WORKFLOW to that workflow's path.
"""

import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import textwrap
import unittest


WORKFLOW = Path(os.environ.get(
    "FAULTLINE_RELEASE_WORKFLOW", Path(__file__).parents[1] / "workflows/release.yml"
)).resolve()
ISOLATE = "Isolate cached package artifacts"
INSPECT = "Inspect clean archive identity and landing copy"
SOURCE = "1" * 40
VERSION = "0.0.1"
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
NAMES = ["faultline-codes", "faultline-types", "faultline-localization",
         "faultline-surface", "faultline-ports", "faultline-app", "faultline-git",
         "faultline-probe-exec", "faultline-store", "faultline-render",
         "faultline-junit", "faultline-loader", "faultline-sarif", "faultline-cli"]


def inline_python(name):
    """Read the real step, failing closed if its name/heredoc format changes."""
    step = WORKFLOW.read_text().split("      - name: " + name + "\n", 1)[1]
    step = step.split("\n      - ", 1)[0]
    match = re.search(r"^          python3 - <<'PY'\n(.*?)^          PY$",
                      step, re.MULTILINE | re.DOTALL)
    if not match:
        raise AssertionError("Missing inline Python in " + name)
    return textwrap.dedent(match.group(1))


def digest(data):
    return hashlib.sha256(data).hexdigest()


class ReleaseWorkflowContracts(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="faultline-release-test-")
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.0.1"\n')
        (self.root / "README.md").write_bytes(b"Generated landing copy\n")
        (self.root / "LICENSE").write_bytes(b"Generated END OF TERMS AND CONDITIONS\n")

    def tearDown(self):
        self.temporary.cleanup()

    def execute(self, name, success=True):
        result = subprocess.run([sys.executable, "-c", inline_python(name)],
                                cwd=self.root, capture_output=True, text=True, timeout=20,
                                env=dict(os.environ, RELEASE_SOURCE_SHA=SOURCE))
        self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
        if not success and name == INSPECT:
            self.assertFalse((self.root / "target/package/release-receipts.json").exists())
        return result

    def fixtures(self, mutation=None):
        """Synthetic acyclic 14-package graph, with all transitive lock entries."""
        blobs = {}
        for name in NAMES:
            lock = [{"name": name, "version": VERSION}]
            lock += [{"name": dep, "version": VERSION, "source": REGISTRY,
                      "checksum": digest(data)} for dep, data in blobs.items()]
            if mutation and name == "faultline-types":
                mutation(lock[1])
            lock_toml = "version = 4\n" + "".join(
                "\n[[package]]\n" + "".join(k + " = " + json.dumps(v) + "\n"
                                              for k, v in p.items()) for p in lock)
            fields = {"name": name, "version": VERSION, "license": "Apache-2.0",
                      "rust-version": "1.93", "description": "Generated fixture",
                      "repository": "https://github.com/EffortlessMetrics/faultline-rs"}
            manifest = "[package]\n" + "".join(k + " = " + json.dumps(v) + "\n"
                                                 for k, v in fields.items())
            files = {"Cargo.toml": manifest.encode(), "Cargo.lock": lock_toml.encode(),
                     "README.md": (self.root / "README.md").read_bytes(),
                     "LICENSE": (self.root / "LICENSE").read_bytes(),
                     ".cargo_vcs_info.json": json.dumps({"git": {"sha1": SOURCE,
                                                                 "dirty": False}}).encode()}
            stream = io.BytesIO()
            with tarfile.open(fileobj=stream, mode="w") as archive:
                for path, data in files.items():
                    member = tarfile.TarInfo(name + "-" + VERSION + "/" + path)
                    member.size = len(data)
                    archive.addfile(member, io.BytesIO(data))
            blobs[name] = gzip.compress(stream.getvalue(), mtime=0)
        return blobs

    def stage(self, blobs, stale=False):
        package = self.root / "target/package"
        scratch = package / "tmp-crate"
        scratch.mkdir(parents=True, exist_ok=True)
        (package / "release-receipts.json").unlink(missing_ok=True)
        for name, data in blobs.items():
            source = scratch / (name + "-" + VERSION + ".crate")
            source.write_bytes(data)  # Cargo's temporary archive is truncated.
            destination = package / source.name
            if stale:
                destination.write_bytes(b"x" * (len(data) + 19))
            else:
                # Independent negatives start fresh, so a previous subtest's
                # longer fixture cannot hide a lock failure behind a gzip tail.
                destination.unlink(missing_ok=True)
            # Cargo 1.93.1 uplift: O_CREAT|O_RDWR plus copy, without O_TRUNC.
            fd = os.open(destination, os.O_CREAT | os.O_RDWR, 0o600)
            with os.fdopen(fd, "r+b") as output, source.open("rb") as input_file:
                shutil.copyfileobj(input_file, output)
        return package

    def test_clean_graph_receipts_match_full_archive_bytes(self):
        blobs = self.fixtures()
        package = self.stage(blobs)
        self.execute(INSPECT)
        receipt = json.loads((package / "release-receipts.json").read_text())
        self.assertEqual(receipt["source_sha"], SOURCE)
        self.assertEqual({p["name"]: p["sha256"] for p in receipt["packages"]},
                         {name: digest(data) for name, data in blobs.items()})

    def test_nontruncating_uplift_quarantine_and_fresh_generation(self):
        blobs = self.fixtures()
        package = self.stage(blobs, stale=True)
        # The old tar/identity-only inspection accepts these valid first members.
        for path in package.glob("*.crate"):
            with tarfile.open(path, "r:gz") as archive:
                self.assertTrue(archive.getnames())
            name = path.name.removesuffix("-" + VERSION + ".crate")
            self.assertNotEqual(digest(path.read_bytes()), digest(blobs[name]))
        self.execute(INSPECT, False)
        before = {p.relative_to(package): p.read_bytes() for p in package.rglob("*") if p.is_file()}
        inode = package.stat().st_ino
        marker = self.root / "target/debug/build-cache-marker"
        marker.parent.mkdir()
        marker.write_bytes(b"Keep the build cache")
        self.execute(ISOLATE)
        self.assertFalse(package.exists())
        preserved, = (self.root / "target").glob("package-quarantine-*/package")
        self.assertEqual(preserved.stat().st_ino, inode)
        self.assertEqual(before, {p.relative_to(preserved): p.read_bytes()
                                  for p in preserved.rglob("*") if p.is_file()})
        self.assertEqual(marker.read_bytes(), b"Keep the build cache")
        self.stage(blobs)  # New generation; no trimming of contaminated files.
        self.execute(INSPECT)
        self.assertEqual(before, {p.relative_to(preserved): p.read_bytes()
                                  for p in preserved.rglob("*") if p.is_file()})

    def test_quarantine_is_recoverable_and_cold_cache_is_noop(self):
        self.execute(ISOLATE)
        self.assertFalse((self.root / "target").exists())
        package = self.stage(self.fixtures())
        before = {p.relative_to(package): p.read_bytes() for p in package.rglob("*") if p.is_file()}
        self.execute(ISOLATE)
        preserved, = (self.root / "target").glob("package-quarantine-*/package")
        preserved.rename(package)
        self.assertEqual(before, {p.relative_to(package): p.read_bytes()
                                  for p in package.rglob("*") if p.is_file()})

    def test_symlinked_package_is_rejected_without_moving_its_target(self):
        target = self.root / "target"
        target.mkdir()
        elsewhere = self.root / "untouched"
        elsewhere.mkdir()
        (elsewhere / "marker").write_bytes(b"Untouched")
        (target / "package").symlink_to(elsewhere, target_is_directory=True)
        self.execute(ISOLATE, False)
        self.assertTrue((target / "package").is_symlink())
        self.assertEqual((elsewhere / "marker").read_bytes(), b"Untouched")

    def test_gzip_suffix_second_member_truncation_and_crc_are_rejected(self):
        blobs = self.fixtures()
        # Mutate a leaf: no other fixture's lock checksum refers to this file,
        # so internal-lock rejection cannot conceal a missing gzip guard.
        original = blobs["faultline-cli"]
        corrupt_crc = bytearray(original)
        corrupt_crc[-8] ^= 1
        for data in [original + b"stale", original + b"\0",
                     original + gzip.compress(b"second member"), original[:-1], bytes(corrupt_crc)]:
            with self.subTest(length=len(data)):
                package = self.stage(blobs)
                (package / "faultline-cli-0.0.1.crate").write_bytes(data)
                self.execute(INSPECT, False)

    def test_internal_checksum_version_and_source_are_rejected(self):
        for field, wrong in [("checksum", "0" * 64), ("version", "0.0.2"),
                             ("source", "registry+https://example.invalid/index")]:
            with self.subTest(field=field):
                self.stage(self.fixtures(lambda dep: dep.update({field: wrong})))
                self.execute(INSPECT, False)

    def test_workflow_runs_contracts_and_isolates_after_cache_before_packaging(self):
        workflow = WORKFLOW.read_text()
        contracts = workflow.index("      - name: Test release workflow contracts\n")
        isolation = workflow.index("      - name: " + ISOLATE + "\n")
        package = workflow.index("      - name: Verify package graph\n")
        self.assertLess(workflow.index("uses: actions/cache@v4"), isolation)
        self.assertLess(workflow.index("      - name: Release checks\n"), isolation)
        self.assertLess(contracts, isolation)
        self.assertLess(isolation, package)
        self.assertIn("run: python3 .github/tests/test_release_workflow.py", workflow)
        self.assertIn("            target\n", workflow)


if __name__ == "__main__":
    unittest.main(verbosity=2)
