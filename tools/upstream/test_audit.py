import json
import subprocess
import tempfile
import unittest
from pathlib import Path

import audit


class AuditTests(unittest.TestCase):
    def test_ownership_is_explicit(self):
        manifest = {"fork_only": ["adapter/**"], "overlays": ["web/**"], "generated": ["en.tsv"], "bridges": {"app.rs": "hook"}}
        for path, expected in [("adapter/foo.rs", "fork-only"), ("web/app.js", "overlay"), ("en.tsv", "generated"), ("app.rs", "bridge"), ("other.rs", "review")]:
            self.assertEqual(audit.ownership(path, manifest), expected)

    def test_boundary_missing_hook_and_leaked_implementation_fail(self):
        with self.directory() as directory:
            root = Path(directory)
            (root / "bridge").write_text("hook()", encoding="utf-8")
            manifest = {"checks": [{"path": "bridge", "required": [r"hook\(\)"], "forbidden": ["leaked"]}]}
            self.assertEqual(audit.check(root, manifest), [])
            (root / "bridge").write_text("leaked", encoding="utf-8")
            self.assertEqual(len(audit.check(root, manifest)), 2)
            (root / "bridge").unlink()
            self.assertIn("missing", audit.check(root, manifest)[0])

    def test_repository_boundary_manifest(self):
        manifest = json.loads(audit.MANIFEST.read_text(encoding="utf-8"))
        self.assertEqual(audit.check(audit.ROOT, manifest), [])

    @staticmethod
    def directory():
        parent = audit.ROOT / "target" / "upstream-audit-tests"
        parent.mkdir(parents=True, exist_ok=True)
        return tempfile.TemporaryDirectory(prefix="fixture-", dir=parent)

    def fixture(self, root):
        self.run_git(root, "init", "-q")
        for name in ("conflict.txt", "clean.txt", "binary.bin", "deleted.txt"):
            (root / name).write_bytes(b"first\nmiddle\nlast\n" if name != "binary.bin" else b"\0base")
        self.run_git(root, "add", ".")
        self.run_git(root, "commit", "-qm", "base")
        base = self.run_git(root, "rev-parse", "HEAD").strip()
        self.run_git(root, "checkout", "-qb", "incoming")
        (root / "conflict.txt").write_text("incoming\nmiddle\nlast\n", encoding="utf-8")
        (root / "clean.txt").write_text("first\nmiddle\nincoming\n", encoding="utf-8")
        (root / "binary.bin").write_bytes(b"\0incoming")
        (root / "deleted.txt").write_text("incoming", encoding="utf-8")
        self.run_git(root, "add", ".")
        self.run_git(root, "commit", "-qm", "incoming")
        self.run_git(root, "checkout", "-qb", "fork", base)
        (root / "conflict.txt").write_text("local\nmiddle\nlast\n", encoding="utf-8")
        (root / "clean.txt").write_text("local\nmiddle\nlast\n", encoding="utf-8")
        (root / "binary.bin").write_bytes(b"\0local")
        (root / "deleted.txt").unlink()
        (root / "new.txt").write_text("not committed", encoding="utf-8")

    @staticmethod
    def run_git(root, *args):
        result = subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "-c", "core.autocrlf=false", *args], cwd=root, capture_output=True, check=True)
        return result.stdout.decode("utf-8")

    def test_dirty_worktree_preview_and_no_staging_or_commits(self):
        with self.directory() as directory:
            root = Path(directory)
            self.fixture(root)
            # Preview artifacts must be ignored just like the real repo's target/.
            (root / ".git" / "info" / "exclude").write_text("target/\n", encoding="utf-8")
            status = self.run_git(root, "status", "--porcelain")
            index = (root / ".git" / "index").read_bytes()
            head = self.run_git(root, "rev-parse", "HEAD")
            result = audit.report(root, "incoming", {}, with_preview=True)
            previews = {item["path"]: item["preview"] for item in result["overlaps"]}
            self.assertEqual(previews["conflict.txt"], "text conflict")
            self.assertEqual(previews["clean.txt"], "text clean")
            self.assertEqual(previews["binary.bin"], "manual: binary")
            self.assertEqual(previews["deleted.txt"], "manual: add/delete/type change")
            self.assertTrue(result["dirty"])
            self.assertEqual(result["local_files"], 5)
            self.assertEqual(result["commits"], {"fork": 0, "upstream": 1})
            self.assertEqual(self.run_git(root, "status", "--porcelain"), status)
            self.assertEqual((root / ".git" / "index").read_bytes(), index)
            self.assertEqual(self.run_git(root, "rev-parse", "HEAD"), head)


if __name__ == "__main__":
    unittest.main()
