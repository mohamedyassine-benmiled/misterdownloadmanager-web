"""Fork-owned catalog sources; extract.py retains only discovery and this collector hook."""
import html
import re


def collect(out, root, files, split_args, literal):
    # Macro arguments are not ordinary i18n calls. The generic extractor still handles
    # mark/t/fmt calls in Rust modules, including this fork's nested adapters.
    for file in files:
        if file.name != "web.rs":
            continue
        source = file.read_text(encoding="utf-8")
        for match in re.finditer(r"\bfield!\s*\(", source):
            spans, _ = split_args(source, match.end() - 1)
            for index in (1, 2):
                text = literal(source, spans[index]) if len(spans) > index else None
                if text:
                    out.add(("", text), "web settings label", file.name)
    web = root / "crates" / "remote" / "web"
    markup = (web / "index.html").read_text(encoding="utf-8")
    script = (web / "app.js").read_text(encoding="utf-8")
    for text in re.findall(r'data-t(?:a|p)?="([^"]+)"', markup):
        out.add(("", html.unescape(text)), "web interface text", "remote/web/index.html")
    for text in re.findall(r"\b(?:T|F)\('([^'\\]*(?:\\.[^'\\]*)*)'", script):
        out.add(("", text.replace("\\'", "'").replace("\\n", "\n")), "web interface text", "remote/web/app.js")
