"""Webmister's fork-owned packaging and coordinated-release checks. Never publishes itself."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[2]
BRAND = "Webmister Download Manager"
PROGRAMS = ("misterdownloadmanager", "misterdownloadmanager-cli", "misterdownloadmanager-nmhost", "misterdownloadmanager-mcp")
EXTENSION_SOURCES = ("remote.js", "background.js", "video.js", "page.js", "popup.html", "popup.js", "popup.css")
VERSION = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*))?")


def version(value):
    match = VERSION.fullmatch(value)
    if not match or any(int(n) > 65535 for n in match.group(1, 2, 3)):
        raise ValueError("version must be X.Y.Z[-prerelease], with browser-compatible components (0–65535)")
    return value


def engine_version():
    return tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]


def commit():
    value = os.environ.get("GITHUB_SHA") or subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if not re.fullmatch(r"[0-9a-f]{40,64}", value):
        raise ValueError("invalid source commit")
    return value


def names(release_version):
    value = version(release_version)
    return {
        "windows": f"Webmister-{value}-windows-x86_64.zip",
        "linux": f"Webmister-{value}-linux-x86_64.tar.gz",
        "chromium": f"Webmister-{value}-extension-chromium.zip",
        "firefox": f"Webmister-{value}-extension-firefox.xpi",
    }


def metadata(release_version=""):
    ref = os.environ.get("GITHUB_REF", "")
    tagged = ref.startswith("refs/tags/web-v")
    value = version(ref.removeprefix("refs/tags/web-v") if tagged else release_version or engine_version())
    if tagged and release_version and release_version != value:
        raise ValueError("input version differs from the Web release tag")
    data = {"brand": BRAND, "version": value, "engine_version": engine_version(), "commit": commit(),
            "publish": tagged and os.environ.get("GITHUB_EVENT_NAME") == "push",
            "prerelease": "-" in value, "tag": f"web-v{value}"}
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            for key, item in data.items():
                output.write(f"{key}={str(item).lower() if isinstance(item, bool) else item}\n")
    return data


def safe_name(name):
    path = PurePosixPath(name)
    return not path.is_absolute() and ".." not in path.parts and "\\" not in name and ":" not in name


def inspect_extension(path, kind, expected_version=None):
    with zipfile.ZipFile(path) as archive:
        entries = archive.namelist()
        if len(entries) != len(set(entries)) or not all(safe_name(name) for name in entries):
            raise ValueError("unsafe or duplicate extension entries")
        manifest = json.loads(archive.read("manifest.json"))
        if manifest["name"] != BRAND:
            raise ValueError("wrong extension brand")
        if expected_version and manifest["version"] != expected_version.split("-", 1)[0]:
            raise ValueError("wrong extension version")
        if expected_version and manifest.get("version_name") != f"{expected_version} (MDM {engine_version()})":
            raise ValueError("missing native-engine compatibility version")
        for file in ["remote.js", "background.js", "popup.html", "popup.js", "icons/webmister-32.png"]:
            if not archive.read(file):
                raise ValueError(f"missing extension resource {file}")
        for file in EXTENSION_SOURCES:
            if archive.read(file) != (ROOT / "extension" / file).read_bytes():
                raise ValueError(f"stale or modified extension source: {file}")
        resources = [*manifest.get("icons", {}).values(), manifest["action"]["default_popup"]]
        resources.extend(file for script in manifest["content_scripts"] for file in script["js"])
        for file in resources:
            if not safe_name(file) or not archive.read(file):
                raise ValueError(f"invalid extension manifest resource: {file}")
        background = archive.read("background.js").decode("utf-8")
        if kind == "firefox":
            if manifest["background"]["scripts"][:2] != ["remote.js", "background.js"]:
                raise ValueError("Firefox adapter loading order changed")
            if manifest["browser_specific_settings"]["gecko"]["id"] != "integration@misterdownloadmanager.com":
                raise ValueError("Firefox compatibility ID changed")
        elif 'importScripts("remote.js")' not in background or not manifest.get("key"):
            raise ValueError("Chromium adapter or compatibility key missing")
        return manifest


def extensions(release_version, source, output):
    version(release_version)
    output.mkdir(parents=True, exist_ok=True)
    for kind, original in [("chromium", "mdm-chromium.zip"), ("firefox", "mdm-firefox.xpi")]:
        path = source / original
        inspect_extension(path, kind)
        destination = output / names(release_version)[kind]
        with zipfile.ZipFile(path) as old, zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as new:
            for info in sorted(old.infolist(), key=lambda entry: entry.filename):
                content = old.read(info.filename)
                if info.filename == "manifest.json":
                    manifest = json.loads(content)
                    manifest["version"] = release_version.split("-", 1)[0]
                    manifest["version_name"] = f"{release_version} (MDM {engine_version()})"
                    content = (json.dumps(manifest, indent=2) + "\n").encode()
                new.writestr(info, content)
        inspect_extension(destination, kind, release_version)


def package(release_version, platform, binaries, extension_dir, output, source_commit):
    version(release_version)
    if platform not in ("windows", "linux"):
        raise ValueError("unsupported release platform")
    for kind in ("chromium", "firefox"):
        inspect_extension(extension_dir / names(release_version)[kind], kind, release_version)
    output.mkdir(parents=True, exist_ok=True)
    # Only explicit public inputs: never recursively package target/ or a runtime data folder.
    with tempfile.TemporaryDirectory(prefix="package-", dir=output) as temporary:
        folder = Path(temporary) / f"Webmister-{release_version}-{platform}-x86_64"
        folder.mkdir()
        for program in PROGRAMS:
            source = binaries / (program + (".exe" if platform == "windows" else ""))
            if not source.is_file() or source.stat().st_size == 0:
                raise ValueError(f"missing compiled program: {source.name}")
            shutil.copy2(source, folder / source.name)
        for source in (ROOT / "LICENSE", ROOT / "ATTRIBUTION.md", ROOT / "brand" / "webmister.svg"):
            shutil.copy2(source, folder / source.name)
        for kind in ("chromium", "firefox"):
            shutil.copy2(extension_dir / names(release_version)[kind], folder / names(release_version)[kind])
        (folder / "data").mkdir()
        (folder / "data" / "README.txt").write_text("Portable server state is stored here. Keep this folder private and back it up before upgrades. No account or credentials are included.\n", encoding="utf-8")
        (folder / "WEBMISTER.json").write_text(json.dumps({"brand": BRAND, "version": release_version, "engine_version": engine_version(), "commit": source_commit, "platform": platform}, indent=2) + "\n", encoding="utf-8")
        (folder / "README.txt").write_text(
            f"{BRAND} {release_version}\nIndependent Web-focused fork of Mister Download Manager. GPL-3.0-or-later.\n"
            "Source and instructions: https://github.com/mohamedyassine-benmiled/misterdownloadmanager-web\n"
            "Run webmister.cmd (Windows) or ./webmister (Linux), with --port 8463 --webhost 127.0.0.1.\n"
            "Use --data and --downloads for explicit server folders. Protect the data folder.\n"
            "Open the logged HTTPS address, verify its certificate fingerprint, then create the owner account with the logged code.\n"
            "The underlying executables retain their MDM names. Upgrade this fork manually from its own releases; no fork auto-updater is provided.\n"
            "Extension ZIPs are unpacked manually; the unsigned Firefox XPI is for temporary developer loading unless independently signed.\n",
            encoding="utf-8")
        if platform == "windows":
            (folder / "webmister.cmd").write_bytes(b'@echo off\r\n"%~dp0misterdownloadmanager.exe" --web %*\r\n')
            destination = output / names(release_version)[platform]
            with zipfile.ZipFile(destination, "w", zipfile.ZIP_DEFLATED) as archive:
                for file in sorted(folder.rglob("*")):
                    if file.is_file():
                        archive.write(file, file.relative_to(folder.parent).as_posix())
        else:
            launcher = folder / "webmister"
            launcher.write_text('#!/bin/sh\nset -eu\nhere=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\nexec "$here/misterdownloadmanager" --web "$@"\n', encoding="utf-8")
            launcher.chmod(0o755)
            for program in PROGRAMS:
                (folder / program).chmod(0o755)
            destination = output / names(release_version)[platform]
            with tarfile.open(destination, "w:gz") as archive:
                def permissions(item):
                    item.uid = item.gid = 0
                    item.uname = item.gname = ""
                    if item.isfile():
                        item.mode = 0o755 if PurePosixPath(item.name).name in (*PROGRAMS, "webmister") else 0o644
                    else:
                        item.mode = 0o700 if PurePosixPath(item.name).name == "data" else 0o755
                    return item
                archive.add(folder, arcname=folder.name, filter=permissions)
        inspect_bundle(destination, platform, release_version, source_commit)
        return destination


def inspect_bundle(path, platform, release_version, source_commit):
    if platform == "windows":
        with zipfile.ZipFile(path) as archive:
            entries = archive.namelist()
            if len(entries) != len(set(entries)) or not all(safe_name(name) for name in entries):
                raise ValueError("unsafe binary archive")
            roots = {name.split("/")[0] for name in entries}
            if len(roots) != 1:
                raise ValueError("binary archive must have one root folder")
            prefix = roots.pop() + "/"
            data = json.loads(archive.read(prefix + "WEBMISTER.json"))
            for program in PROGRAMS:
                if not archive.read(prefix + program + ".exe"):
                    raise ValueError("empty program")
    else:
        with tarfile.open(path) as archive:
            entries = archive.getmembers()
            if not all(safe_name(item.name) and not item.issym() and not item.islnk() for item in entries):
                raise ValueError("unsafe Linux archive")
            roots = {item.name.split("/")[0] for item in entries}
            if len(roots) != 1:
                raise ValueError("binary archive must have one root folder")
            prefix = roots.pop() + "/"
            data = json.load(archive.extractfile(prefix + "WEBMISTER.json"))
            for program in (*PROGRAMS, "webmister"):
                item = archive.getmember(prefix + program)
                if item.size == 0 or not item.mode & 0o111:
                    raise ValueError("Linux program/launcher is not executable")
    if any(data[key] != expected for key, expected in [("brand", BRAND), ("version", release_version), ("engine_version", engine_version()), ("commit", source_commit), ("platform", platform)]):
        raise ValueError("bundle provenance/version mismatch")


def verify(release_version, output, source_commit, published=False):
    expected = names(release_version)
    for name in [*expected.values(), "image-release.json"]:
        if not (output / name).is_file():
            raise ValueError(f"release incomplete: missing {name}")
    for platform in ("windows", "linux"):
        inspect_bundle(output / expected[platform], platform, release_version, source_commit)
    for kind in ("chromium", "firefox"):
        inspect_extension(output / expected[kind], kind, release_version)
    image = json.loads((output / "image-release.json").read_text(encoding="utf-8"))
    if image["commit"] != source_commit:
        raise ValueError("Docker image and native downloads are from different commits")
    if published and (not image.get("publish") or image.get("version") != release_version or not image.get("digests")):
        raise ValueError("Docker publication/digest is required before releasing downloads")
    if published and not all(re.fullmatch(re.escape(image["image"]) + r"@sha256:[0-9a-f]{64}", digest) for digest in image["digests"]):
        raise ValueError("invalid published image digest")
    receipt = {"brand": BRAND, "version": release_version, "engine_version": engine_version(), "commit": source_commit,
               "platforms": ["windows-x86_64", "linux-x86_64"], "files": expected, "image": image,
               "unsigned_downloads": True, "firefox_xpi_signed": False}
    (output / "RELEASE.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    files = [*expected.values(), "image-release.json", "RELEASE.json"]
    (output / "SHA256SUMS.txt").write_text("".join(f"{hashlib.sha256((output / name).read_bytes()).hexdigest()}  {name}\n" for name in sorted(files)), encoding="ascii")
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["metadata", "extensions", "package", "verify"])
    parser.add_argument("--version", default="")
    parser.add_argument("--platform", choices=["windows", "linux"])
    parser.add_argument("--bin", type=Path, default=ROOT / "target" / "release")
    parser.add_argument("--source", type=Path, default=ROOT / "target" / "extension")
    parser.add_argument("--out", type=Path, default=ROOT / "target" / "webmister-dist")
    parser.add_argument("--published", action="store_true")
    args = parser.parse_args()
    try:
        if args.operation == "metadata":
            print(json.dumps(metadata(args.version), indent=2))
        elif args.operation == "extensions":
            extensions(args.version, args.source, args.out)
        elif args.operation == "package":
            print(package(args.version, args.platform, args.bin, args.source, args.out, commit()))
        else:
            verify(args.version, args.out, commit(), args.published)
            print("Coordinated release verified: Windows, Linux, both extensions, image metadata and checksums.")
    except (ValueError, OSError, KeyError, zipfile.BadZipFile) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
