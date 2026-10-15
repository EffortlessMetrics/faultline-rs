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
SCENARIO_INDEX = Path(__file__).parents[2] / "docs/scenarios/scenario_index.md"
ISOLATE = "Isolate cached package artifacts"
INSPECT = "Inspect clean archive identity and landing copy"
SELECT = "Validate release selection and tag"
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


def git(root, *args):
    return subprocess.check_output(["git", "-c", "commit.gpgsign=false", "-c",
                                    "tag.gpgsign=false", *args], cwd=root,
                                   text=True, stderr=subprocess.DEVNULL).strip()


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
        self.assertIn("          python3 .github/tests/test_release_workflow.py\n", workflow)
        self.assertIn("            target\n", workflow)
        # The Rust atlas scanner only discovers Rust tests under crates/.
        # Keep Python scenarios cataloged and checked by their own suite.
        scenarios = SCENARIO_INDEX.read_text()
        for name in unittest.defaultTestLoader.getTestCaseNames(type(self)):
            self.assertIn("| Python: `" + name + "` |", scenarios)

    def git_fixture(self):
        """A real local origin, immutable release tag and newer controller main."""
        upstream = self.root / "upstream"
        upstream.mkdir()
        members = NAMES + ["faultline-fixtures", "xtask"]
        (upstream / "Cargo.toml").write_text(
            "[workspace]\nmembers = " + json.dumps(members) +
            '\n[workspace.package]\nversion = "0.0.1"\n')
        for name in members:
            (upstream / name).mkdir()
            (upstream / name / "Cargo.toml").write_text(
                '[package]\nname = ' + json.dumps(name) + '\nversion.workspace = true\n' +
                ('publish = false\n' if name in ["faultline-fixtures", "xtask"] else ''))
        for path, data in {".github/workflows/release.yml": "obsolete release controls\n",
                           ".github/tests/test_release_workflow.py": "print('obsolete tests executed')\n",
                           "docs/scenarios/scenario_index.md": "obsolete scenarios\n"}.items():
            destination = upstream / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(data)
        git(upstream, "init", "--initial-branch=main")
        git(upstream, "config", "user.name", "Generated fixture")
        git(upstream, "config", "user.email", "fixture@example.invalid")
        git(upstream, "add", ".")
        git(upstream, "commit", "-m", "Release source")
        source = git(upstream, "rev-parse", "HEAD")
        tree = git(upstream, "rev-parse", "HEAD^{tree}")
        git(upstream, "tag", "-a", "v0.0.1", "-m", "v0.0.1")
        tag = git(upstream, "rev-parse", "refs/tags/v0.0.1")
        (upstream / ".github/workflows/release.yml").write_text("fixed controller\n")
        (upstream / "docs/scenarios/scenario_index.md").write_text("fixed scenarios\n")
        (upstream / ".github/tests/test_release_workflow.py").write_text(
            "from pathlib import Path\n" +
            "assert Path('.github/workflows/release.yml').read_text() == 'fixed controller\\n'\n" +
            "assert Path('docs/scenarios/scenario_index.md').read_text() == 'fixed scenarios\\n'\n" +
            "print('FIXED_CONTROLLER_TESTS')\n")
        git(upstream, "add", ".")
        git(upstream, "commit", "-m", "Fixed controller")
        controller = git(upstream, "rev-parse", "HEAD")
        remote = self.root / "remote.git"
        git(self.root, "clone", "--bare", str(upstream), str(remote))
        runner = self.root / "runner"
        runner.mkdir()
        git(runner, "init")
        git(runner, "remote", "add", "origin", str(remote))
        git(runner, "fetch", "--force", "origin",
            "+refs/heads/*:refs/remotes/origin/*", "+refs/tags/*:refs/tags/*")
        git(runner, "checkout", "--detach", source)
        return {"runner": runner, "remote": remote, "source": source, "tree": tree,
                "tag": tag, "controller": controller}

    def select(self, fixture, event="workflow_dispatch", publish="false", success=True, **changes):
        output = self.root / "selection-output"
        output.unlink(missing_ok=True)
        environment = dict(os.environ, GITHUB_EVENT_NAME=event, GITHUB_OUTPUT=str(output),
                           GITHUB_REF_TYPE="branch" if event == "workflow_dispatch" else "tag",
                           GITHUB_REF="refs/heads/main" if event == "workflow_dispatch" else "refs/tags/v0.0.1",
                           GITHUB_REF_NAME="main" if event == "workflow_dispatch" else "v0.0.1",
                           GITHUB_SHA=fixture["controller"] if event == "workflow_dispatch" else fixture["source"],
                           GITHUB_WORKFLOW_SHA=fixture["controller"],
                           GITHUB_WORKFLOW_REF="EffortlessMetrics/faultline-rs/.github/workflows/release.yml@refs/heads/main",
                           RECOVERY_SOURCE_SHA=fixture["source"], RECOVERY_SOURCE_TREE=fixture["tree"],
                           RECOVERY_TAG_OBJECT=fixture["tag"], RECOVERY_PUBLISH=publish)
        environment.update(changes)
        result = subprocess.run([sys.executable, "-c", inline_python(SELECT)],
                                cwd=fixture["runner"], capture_output=True, text=True,
                                env=environment, timeout=20)
        self.assertEqual(result.returncode == 0, success, result.stdout + result.stderr)
        if not success:
            self.assertFalse(output.exists(), "Failure must not authorize upload or source receipts")
            return {}
        return dict(line.split("=", 1) for line in output.read_text().splitlines())

    def test_annotated_checkout_fallback_is_red_and_explicit_sha_is_green(self):
        fixture = self.git_fixture()
        runner, remote = fixture["runner"], fixture["remote"]
        remote_before = git(runner, "ls-remote", "origin", "refs/tags/v0.0.1*")
        # checkout's implicit tag+commit testRef compares the raw object to the
        # event commit, then force-fetches the commit into the runner-local tag.
        self.assertNotEqual(git(runner, "rev-parse", "refs/tags/v0.0.1"), fixture["source"])
        git(runner, "fetch", "--no-tags", "--force", "origin",
            "+" + fixture["source"] + ":refs/tags/v0.0.1")
        self.assertEqual(git(runner, "cat-file", "-t", "refs/tags/v0.0.1"), "commit")
        self.select(fixture, event="push", success=False)
        # SHA-only checkout has no tag ref to overwrite; full history retains A.
        git(runner, "fetch", "--force", "origin", "+refs/tags/*:refs/tags/*")
        self.assertEqual(git(runner, "cat-file", "-t", "refs/tags/v0.0.1"), "tag")
        self.assertEqual(self.select(fixture, event="push"),
                         {"source_sha": fixture["source"], "publish": "true"})
        self.assertEqual(git(runner, "ls-remote", "origin", "refs/tags/v0.0.1*"), remote_before)
        self.assertEqual(git(remote, "rev-parse", "refs/tags/v0.0.1"), fixture["tag"])
        git(runner, "-c", "user.name=Generated fixture", "-c", "user.email=fixture@example.invalid",
            "tag", "-a", "v0.0.2", fixture["source"], "-m", "Wrong version")
        self.select(fixture, event="push", success=False, GITHUB_REF="refs/tags/v0.0.2")
        git(runner, "-c", "user.name=Generated fixture", "-c", "user.email=fixture@example.invalid",
            "tag", "--force", "-a", "v0.0.1", fixture["controller"], "-m", "Wrong commit")
        self.select(fixture, event="push", success=False)
        git(runner, "update-ref", "refs/tags/v0.0.1", fixture["tag"])

    def test_recovery_pins_and_main_push_publication_gate_fail_closed(self):
        fixture = self.git_fixture()
        self.assertEqual(self.select(fixture), {"source_sha": fixture["source"], "publish": "false"})
        self.assertEqual(self.select(fixture, publish="true")["publish"], "true")
        self.assertEqual(self.select(fixture, event="push", GITHUB_REF_TYPE="branch",
                                     GITHUB_REF="refs/heads/main")["publish"], "false")
        for changes in [{"RECOVERY_SOURCE_SHA": "0" * 40}, {"RECOVERY_SOURCE_TREE": "0" * 40},
                        {"RECOVERY_TAG_OBJECT": "0" * 40}, {"GITHUB_REF": "refs/heads/release/other"},
                        {"GITHUB_WORKFLOW_SHA": "0" * 40}, {"GITHUB_SHA": fixture["source"]},
                        {"GITHUB_SHA": fixture["source"], "GITHUB_WORKFLOW_SHA": fixture["source"]},
                        {"GITHUB_WORKFLOW_REF": "other/repo/.github/workflows/release.yml@refs/heads/main"},
                        {"RECOVERY_PUBLISH": "yes"}]:
            with self.subTest(changes=changes):
                self.select(fixture, success=False, **changes)
        self.select(fixture, event="push", success=False, GITHUB_REF_TYPE="branch",
                    GITHUB_REF="refs/heads/main", GITHUB_SHA=fixture["controller"])
        runner = fixture["runner"]
        (runner / "Cargo.toml").write_text((runner / "Cargo.toml").read_text() + "# dirty\n")
        self.select(fixture, success=False)
        git(runner, "checkout", "--", "Cargo.toml")
        git(runner, "update-ref", "refs/tags/v0.0.1", fixture["source"])
        self.select(fixture, success=False)
        self.select(fixture, event="push", success=False)
        git(runner, "update-ref", "refs/tags/v0.0.1", fixture["tag"])
        unrelated = git(runner, "-c", "user.name=Generated fixture", "-c",
                        "user.email=fixture@example.invalid", "commit-tree", fixture["tree"],
                        "-m", "Unrelated main")
        git(runner, "update-ref", "refs/remotes/origin/main", unrelated)
        self.select(fixture, event="push", success=False)
        self.select(fixture, success=False, GITHUB_SHA=unrelated, GITHUB_WORKFLOW_SHA=unrelated)

    def test_fixed_controller_tests_run_before_immutable_source_selection(self):
        fixture = self.git_fixture()
        runner = fixture["runner"]
        workflow = WORKFLOW.read_text()
        start = workflow.index("      - name: Test release workflow contracts\n")
        end = workflow.index("\n      - ", start + 1)
        test_step = textwrap.dedent(workflow[start:end].split("        run: |\n", 1)[1])
        self.assertLess(start, workflow.index("      - name: Select immutable v0.0.1 recovery source\n"))
        git(runner, "checkout", "--detach", fixture["controller"])
        result = subprocess.run(["bash", "-e", "-c", test_step], cwd=runner,
                                capture_output=True, text=True, timeout=20,
                                env=dict(os.environ, GITHUB_WORKFLOW_SHA=fixture["controller"]))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("FIXED_CONTROLLER_TESTS", result.stdout)
        git(runner, "checkout", "--detach", "refs/tags/v0.0.1")
        self.assertIn("obsolete", (runner / ".github/tests/test_release_workflow.py").read_text())
        wrong_controller = subprocess.run(
            ["bash", "-e", "-c", test_step], cwd=runner, capture_output=True,
            text=True, timeout=20,
            env=dict(os.environ, GITHUB_WORKFLOW_SHA=fixture["controller"]))
        self.assertNotEqual(wrong_controller.returncode, 0)
        self.assertNotIn("obsolete tests executed", wrong_controller.stdout)
        self.assertEqual(self.select(fixture)["source_sha"], fixture["source"])
        self.assertEqual(git(runner, "rev-parse", "HEAD"), fixture["source"])
        self.assertEqual(git(runner, "status", "--porcelain", "--untracked-files=no"), "")

    def test_recovery_workflow_wiring_preserves_pins_token_scope_and_concurrency(self):
        workflow = WORKFLOW.read_text()
        for pin in ["RECOVERY_SOURCE_SHA: a2c59ef1cedfee120762a4e7727b62224e57a8e7",
                    "RECOVERY_SOURCE_TREE: 66deea4bc3e0a490eda8e08f4392dead3f62c177",
                    "RECOVERY_TAG_OBJECT: bd1d904ef84f6f0bc6f2b361de9bc6931bc74fa0"]:
            self.assertIn(pin, workflow)
        self.assertIn("          ref: ${{ github.sha }}\n", workflow)
        self.assertIn("          ref: refs/tags/v0.0.1\n", workflow)
        self.assertEqual(workflow.count("persist-credentials: false"), 2)
        self.assertIn("default: false", workflow)
        self.assertIn("group: release-check-${{ github.event_name == 'workflow_dispatch' && 'refs/tags/v0.0.1' || github.ref }}", workflow)
        self.assertIn("RELEASE_SOURCE_SHA: ${{ steps.selection.outputs.source_sha }}", workflow)
        self.assertIn("name: crates-source-${{ steps.selection.outputs.source_sha }}", workflow)
        self.assertEqual(workflow.count("if: steps.selection.outputs.publish == 'true'"), 3)
        self.assertEqual(workflow.count("${{ secrets.CARGO_REGISTRY_TOKEN }}"), 1)
        upload = workflow.split("      - name: Publish qualified workspace to crates.io\n", 1)[1]
        self.assertIn("${{ secrets.CARGO_REGISTRY_TOKEN }}", upload.split("\n      - ", 1)[0])


if __name__ == "__main__":
    unittest.main(verbosity=2)
