import hashlib
import json
import os
import subprocess
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import release


class CoordinatedReleaseTests(unittest.TestCase):
    def setUp(self):
        parent = release.ROOT / "target" / "web-release-tests"
        parent.mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(prefix="fixture-", dir=parent)
        self.root = Path(self.temp.name)
        self.source = self.root / "original-extensions"
        self.out = self.root / "release"
        self.source.mkdir()
        self.out.mkdir()
        self.sha = "a" * 40

    def tearDown(self):
        self.temp.cleanup()

    def fixture_extensions(self):
        for kind, manifest_file, archive_name in [("chromium", "manifest.json", "mdm-chromium.zip"), ("firefox", "manifest.firefox.json", "mdm-firefox.xpi")]:
            manifest = json.loads((release.ROOT / "extension" / manifest_file).read_text(encoding="utf-8"))
            files = set(release.EXTENSION_SOURCES) | set(manifest["icons"].values())
            with zipfile.ZipFile(self.source / archive_name, "w") as archive:
                archive.writestr("manifest.json", json.dumps(manifest))
                for file in sorted(files):
                    archive.write(release.ROOT / "extension" / file, file)
        release.extensions("1.2.3", self.source, self.out)

    def fixtures(self):
        self.fixture_extensions()
        for platform in ("windows", "linux"):
            binaries = self.root / platform
            binaries.mkdir()
            for program in release.PROGRAMS:
                (binaries / (program + (".exe" if platform == "windows" else ""))).write_bytes(b"Synthetic unit-test binary, never a release input.")
            release.package("1.2.3", platform, binaries, self.out, self.out, self.sha)
        self.image = {"image": "ghcr.io/owner/repo", "commit": self.sha, "version": "1.2.3", "publish": False, "digests": []}
        self.write_image()

    def write_image(self):
        (self.out / "image-release.json").write_text(json.dumps(self.image), encoding="utf-8")

    def test_browser_compatible_versions_and_unsafe_paths(self):
        for value in ("1.2.3", "1.2.3-rc.1", "0.0.1"):
            self.assertEqual(release.version(value), value)
        for value in ("v1.2.3", "1.2", "1.2.3;bad", "65536.0.0", "01.2.3", "../release"):
            with self.assertRaises(ValueError):
                release.version(value)
        self.assertFalse(release.safe_name("../secret"))
        self.assertFalse(release.safe_name("C:\\private"))

    def test_manual_is_build_only_and_tag_identity_wins(self):
        with patch.dict(os.environ, {"GITHUB_SHA": self.sha, "GITHUB_REF": "refs/heads/main", "GITHUB_EVENT_NAME": "workflow_dispatch"}, clear=True):
            self.assertFalse(release.metadata("1.2.3")["publish"])
        with patch.dict(os.environ, {"GITHUB_SHA": self.sha, "GITHUB_REF": "refs/tags/web-v1.2.3", "GITHUB_EVENT_NAME": "push"}, clear=True):
            self.assertTrue(release.metadata()["publish"])
            with self.assertRaises(ValueError):
                release.metadata("9.9.9")

    def test_both_extensions_are_branded_versioned_and_keep_transport(self):
        self.fixture_extensions()
        for kind in ("chromium", "firefox"):
            manifest = release.inspect_extension(self.out / release.names("1.2.3")[kind], kind, "1.2.3")
            self.assertEqual(manifest["name"], release.BRAND)
            self.assertEqual(manifest["version"], "1.2.3")

    def test_stale_extension_cannot_ship(self):
        self.fixture_extensions()
        path = self.out / release.names("1.2.3")["chromium"]
        with zipfile.ZipFile(path) as old:
            files = {item.filename: old.read(item) for item in old.infolist()}
        files["remote.js"] = b"stale transport"
        with zipfile.ZipFile(path, "w") as archive:
            for name, data in files.items():
                archive.writestr(name, data)
        with self.assertRaisesRegex(ValueError, "stale"):
            release.inspect_extension(path, "chromium", "1.2.3")

    def test_extension_sources_have_cross_platform_checkout_bytes(self):
        for file in release.EXTENSION_SOURCES:
            result = subprocess.run(
                ["git", "check-attr", "eol", "--", f"extension/{file}"],
                cwd=release.ROOT, check=True, capture_output=True, text=True,
            )
            self.assertTrue(result.stdout.strip().endswith(": eol: lf"), result.stdout)

    def test_complete_release_has_correct_checksums_and_no_runtime_state(self):
        self.fixtures()
        receipt = release.verify("1.2.3", self.out, self.sha)
        self.assertEqual(receipt["commit"], self.sha)
        for row in (self.out / "SHA256SUMS.txt").read_text().splitlines():
            digest, name = row.split("  ", 1)
            self.assertEqual(digest, hashlib.sha256((self.out / name).read_bytes()).hexdigest())
        with zipfile.ZipFile(self.out / release.names("1.2.3")["windows"]) as archive:
            self.assertFalse(any(name.endswith(("web-auth.json", "ipc.json", "devices.json", "downloads.json")) for name in archive.namelist()))

    def test_missing_platform_blocks_the_entire_release(self):
        self.fixtures()
        (self.out / release.names("1.2.3")["linux"]).unlink()
        with self.assertRaisesRegex(ValueError, "incomplete"):
            release.verify("1.2.3", self.out, self.sha)

    def test_mismatched_image_commit_and_missing_digest_block_publication(self):
        self.fixtures()
        with self.assertRaisesRegex(ValueError, "publication/digest"):
            release.verify("1.2.3", self.out, self.sha, True)
        self.image["commit"] = "b" * 40
        self.write_image()
        with self.assertRaisesRegex(ValueError, "different commits"):
            release.verify("1.2.3", self.out, self.sha)
        self.image.update(commit=self.sha, publish=True, digests=["ghcr.io/owner/repo@sha256:" + "b" * 64])
        self.write_image()
        release.verify("1.2.3", self.out, self.sha, True)


if __name__ == "__main__":
    unittest.main()
