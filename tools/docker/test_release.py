import unittest
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from release import plan


class ReleasePolicyTests(unittest.TestCase):
    def plan(self, event="push", ref="refs/heads/main", publish=False):
        return plan("Owner/MDM-Web", event, ref, "a" * 40, publish)

    def test_main_publishes_edge_not_latest(self):
        value = self.plan()
        self.assertEqual(value["image"], "ghcr.io/owner/mdm-web")
        self.assertTrue(value["publish"])
        self.assertFalse(value["release"])
        self.assertEqual([tag.split(":")[-1] for tag in value["tags"]], ["sha-" + "a" * 40, "edge"])

    def test_versioned_release_and_prerelease(self):
        stable = self.plan(ref="refs/tags/web-v1.2.3")
        self.assertTrue(stable["release"])
        self.assertIn("ghcr.io/owner/mdm-web:latest", stable["tags"])
        preview = self.plan(ref="refs/tags/web-v1.2.3-rc.1")
        self.assertTrue(preview["publish"])
        self.assertEqual(preview["version"], "1.2.3-rc.1")
        self.assertNotIn("ghcr.io/owner/mdm-web:latest", preview["tags"])

    def test_pr_never_publishes_even_for_main(self):
        self.assertFalse(self.plan(event="pull_request")["publish"])
        with self.assertRaises(ValueError):
            self.plan(event="pull_request", publish=True)

    def test_manual_default_is_build_only_and_publication_is_ref_limited(self):
        self.assertFalse(self.plan(event="workflow_dispatch")["publish"])
        self.assertTrue(self.plan(event="workflow_dispatch", publish=True)["publish"])
        self.assertFalse(self.plan(event="workflow_dispatch", ref="refs/heads/experiment")["publish"])
        with self.assertRaises(ValueError):
            self.plan(event="workflow_dispatch", ref="refs/heads/experiment", publish=True)

    def test_desktop_tags_and_unsafe_inputs_are_rejected(self):
        for tag in ("v1.2.3", "web-vbad", "web-v1.2.3;echo", "web-v01.2.3", "web-v1.2.3+build"):
            with self.assertRaises(ValueError):
                self.plan(ref="refs/tags/" + tag)
        with self.assertRaises(ValueError):
            plan("not a repository", "push", "refs/heads/main", "a" * 40)
        with self.assertRaises(ValueError):
            plan("owner/repo", "push", "refs/heads/main", "not-a-sha")

    def test_workflow_outputs_and_published_digest_receipt(self):
        parent = Path(__file__).resolve().parents[2] / "target" / "docker-release-tests"
        parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="policy-", dir=parent) as directory:
            root = Path(directory)
            output = root / "github-output"
            env = {**os.environ, "GITHUB_REPOSITORY": "Owner/MDM-Web", "GITHUB_EVENT_NAME": "push",
                   "GITHUB_REF": "refs/tags/web-v1.2.3", "GITHUB_SHA": "a" * 40,
                   "GITHUB_OUTPUT": str(output), "MDM_PUBLISH_REQUESTED": "false"}
            command = [sys.executable, str(Path(__file__).with_name("release.py")), "--out", str(root)]
            subprocess.run(command, env=env, capture_output=True, check=True)
            values = output.read_text(encoding="utf-8")
            self.assertIn("publish=true\n", values)
            self.assertIn("tags<<MDM_IMAGE_TAGS\n", values)
            data = json.loads((root / "image-release.json").read_text(encoding="utf-8"))
            self.assertEqual((root / "image-tags.txt").read_text(encoding="utf-8").splitlines(), data["tags"])
            digest = "ghcr.io/owner/mdm-web@sha256:" + "b" * 64
            digests = root / "digests.json"
            digests.write_text(json.dumps([digest]), encoding="utf-8")
            subprocess.run([*command, "--record-digests", str(digests)], env=env, capture_output=True, check=True)
            self.assertEqual(json.loads((root / "image-release.json").read_text(encoding="utf-8"))["digests"], [digest])
            digests.write_text("[]", encoding="utf-8")
            self.assertNotEqual(subprocess.run([*command, "--record-digests", str(digests)], env=env, capture_output=True).returncode, 0)


if __name__ == "__main__":
    unittest.main()
