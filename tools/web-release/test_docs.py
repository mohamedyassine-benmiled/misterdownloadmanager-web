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

    def handle_starttag(self, tag, attrs):
        attributes = dict(attrs)
        if tag == "img" and "src" in attributes:
            self.paths.append(attributes["src"])
        if tag == "source" and "srcset" in attributes:
            self.paths.append(attributes["srcset"])


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
        self.assertEqual(len(data["captures"]), 5)
        for capture in data["captures"]:
            self.assertIn("synthetic", capture["origin"])
            self.assertTrue((release.ROOT / "docs/images/webmister" / capture["file"]).is_file())

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
