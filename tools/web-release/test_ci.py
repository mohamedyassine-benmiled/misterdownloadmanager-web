"""Keep known runner/dependency/publication failures from returning (no YAML dependency)."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class WorkflowContractTests(unittest.TestCase):
    def test_playwright_jobs_use_supported_linux_images(self):
        web = (ROOT / ".github/workflows/web.yml").read_text(encoding="utf-8")
        self.assertNotIn("runs-on: ubuntu-latest", web)
        self.assertIn("runs-on: ubuntu-24.04", web)
        release = (ROOT / ".github/workflows/web-release.yml").read_text(encoding="utf-8")
        self.assertIn("os: ubuntu-22.04", release)  # Native bundle's glibc floor stays unchanged.
        self.assertIn("os: windows-2022", release)

    def test_default_engine_builds_provision_boringssl(self):
        for path in ("docker/web.Dockerfile", ".github/workflows/web.yml", ".github/workflows/web-release.yml"):
            text = (ROOT / path).read_text(encoding="utf-8")
            self.assertIn("cmake", text, path)
            self.assertIn("libclang-dev", text, path)
        release = (ROOT / ".github/workflows/web-release.yml").read_text(encoding="utf-8")
        self.assertIn("choco install nasm", release)
        self.assertIn("LIBCLANG_PATH=", release)
        self.assertNotIn("--no-default-features", release)

    def test_upstream_website_only_deploys_on_upstream(self):
        text = (ROOT / ".github/workflows/pages.yml").read_text(encoding="utf-8")
        self.assertRegex(text, r"deploy:\s+(?:#[^\n]*\n\s*)*if: github.repository == 'anefzaoui/misterdownloadmanager'")
        self.assertIn("python tools/website/build.py", text)  # Forks still validate the site build.

    def test_release_acceptance_and_publication_gates_are_kept(self):
        text = (ROOT / ".github/workflows/web-release.yml").read_text(encoding="utf-8")
        self.assertIn('tags: ["web-v*"]', text)
        self.assertIn("node tools/web/headless.mjs", text)
        self.assertIn("needs: [metadata, extension, native, image]", text)
        self.assertIn("if: needs.metadata.outputs.publish == 'true'", text)
        self.assertNotRegex(text, re.compile(r"continue-on-error:\s*true"))


if __name__ == "__main__":
    unittest.main()
