import hashlib
from html.parser import HTMLParser
import json
import re
import struct
import unittest
from urllib.parse import urlparse, unquote

import release


class Assets(HTMLParser):
    def __init__(self):
        super().__init__()
        self.paths = []
        self.anchor_depth = 0
        self.publication_buttons = []

    def handle_starttag(self, tag, attrs):
        attributes = dict(attrs)
        if tag == "a":
            self.anchor_depth += 1
        if tag == "img" and "src" in attributes:
            self.paths.append(attributes["src"])
            if "img.shields.io/badge/" in attributes["src"] and any(
                label in attributes["src"] for label in ("Download%20for-", "Docker-", "Add%20to-")
            ):
                self.publication_buttons.append((attributes, self.anchor_depth))
        if tag == "source" and "srcset" in attributes:
            self.paths.append(attributes["srcset"])

    def handle_endtag(self, tag):
        if tag == "a":
            self.anchor_depth -= 1


class DocumentationTests(unittest.TestCase):
    def test_readme_local_links_and_images_exist(self):
        text = (release.ROOT / "README.md").read_text(encoding="utf-8")
        assets = Assets()
        assets.feed(text)
        paths = assets.paths + re.findall(r"\[[^\]]+\]\(([^)]+)\)", text)
        for value in paths:
            parsed = urlparse(value)
            if not parsed.scheme and parsed.path:
                self.assertTrue((release.ROOT / unquote(parsed.path)).exists(), value)
        self.assertIn("independent", text)
        self.assertNotIn("anefzaoui/misterdownloadmanager/releases", text)
        self.assertNotIn("target/web-ui-local", text)
        self.assertIn("127.0.0.1:8463:8463", text)

    def test_screenshot_manifest_matches_the_shipped_sources(self):
        data = json.loads((release.ROOT / "docs/images/webmister/captures.json").read_text(encoding="utf-8"))
        for file, expected in data["sources"].items():
            normalized = (release.ROOT / file).read_text(encoding="utf-8").replace("\r\n", "\n").encode()
            self.assertEqual(hashlib.sha256(normalized).hexdigest(), expected, f"Regenerate README screenshots after changing {file}")
        self.assertEqual(len(data["captures"]), 12)
        for capture in data["captures"]:
            self.assertIn("synthetic", capture["origin"])
            self.assertTrue((release.ROOT / "docs/images/webmister" / capture["file"]).is_file())

    def test_unpublished_download_and_store_buttons_stay_disabled(self):
        text = (release.ROOT / "README.md").read_text(encoding="utf-8")
        assets = Assets()
        assets.feed(text)
        self.assertEqual(len(assets.publication_buttons), 5)
        for attributes, anchor_depth in assets.publication_buttons:
            self.assertEqual(anchor_depth, 0, attributes["alt"])
            self.assertIn("-6b7280?", attributes["src"])
            self.assertIn("soon", attributes["src"])
        self.assertNotIn("chromewebstore.google.com/detail/", text)
        self.assertNotIn("addons.mozilla.org/firefox/addon/", text)
        self.assertIn("stargazers", text)

    def test_benchmark_is_explicitly_upstream_not_a_fork_measurement(self):
        text = (release.ROOT / "README.md").read_text(encoding="utf-8")
        self.assertIn("not a Webmister benchmark", text)
        self.assertIn("caa200c", text)
        self.assertIn("not rerun against this fork", text)
        self.assertIn("docs/bench/run5/REPORT.md", text)

    def test_logo_source_and_browser_icon_dimensions(self):
        data = json.loads((release.ROOT / "brand/assets.json").read_text(encoding="utf-8"))
        svg = (release.ROOT / data["source"]).read_text(encoding="utf-8").replace("\r\n", "\n").encode()
        self.assertEqual(hashlib.sha256(svg).hexdigest(), data["source_sha256"])
        for size in (16, 32, 48, 128, 256):
            content = (release.ROOT / f"extension/icons/webmister-{size}.png").read_bytes()
            self.assertEqual(content[:8], b"\x89PNG\r\n\x1a\n")
            self.assertEqual(struct.unpack(">II", content[16:24]), (size, size))


if __name__ == "__main__":
    unittest.main()
