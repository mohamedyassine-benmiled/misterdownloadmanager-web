"""Fork image naming/publication policy. No registry calls or credentials in this script."""
import argparse
import json
import os
import re
from pathlib import Path


def plan(repository, event, ref, sha, manual_publish=False):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("invalid owner/repository")
    if not re.fullmatch(r"[0-9a-f]{40,64}", sha):
        raise ValueError("invalid commit SHA")
    image = f"ghcr.io/{repository.lower()}"
    version = None
    if ref.startswith("refs/tags/"):
        match = re.fullmatch(r"refs/tags/web-v((?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?)", ref)
        if not match:
            raise ValueError("Web images use web-vX.Y.Z[-prerelease] tags, separate from desktop v* releases")
        version = match[1]
        if len(version) > 128:
            raise ValueError("image version is too long")
    trusted_ref = ref == "refs/heads/main" or version is not None
    if event not in ("push", "pull_request", "workflow_dispatch"):
        raise ValueError("unsupported workflow event")
    publish = (event == "push" or (event == "workflow_dispatch" and manual_publish)) and trusted_ref
    if manual_publish and not publish:
        raise ValueError("manual publication requires main or a web-v* release tag")
    tags = [f"{image}:sha-{sha}"]
    if ref == "refs/heads/main":
        tags.append(f"{image}:edge")
    if version:
        tags.append(f"{image}:{version}")
        if "-" not in version:
            tags.append(f"{image}:latest")
    return {"image": image, "tags": tags, "test_tag": tags[0], "version": version,
            "tag": ref.removeprefix("refs/tags/") if version else "", "commit": sha,
            "publish": publish, "release": publish and version is not None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record-digests", type=Path)
    parser.add_argument("--out", type=Path, default=Path("target/docker-release"))
    args = parser.parse_args()
    if args.record_digests:
        record = args.out / "image-release.json"
        data = json.loads(record.read_text(encoding="utf-8"))
        digests = json.loads(args.record_digests.read_text(encoding="utf-8"))
        if not digests or not all(re.fullmatch(re.escape(data["image"]) + r"@sha256:[0-9a-f]{64}", digest) for digest in digests):
            parser.error("registry did not return the published image's SHA-256 digest")
        data["digests"] = digests
        record.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
        return
    try:
        data = plan(os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_EVENT_NAME"],
                    os.environ["GITHUB_REF"], os.environ["GITHUB_SHA"],
                    os.environ.get("MDM_PUBLISH_REQUESTED") == "true")
    except (KeyError, ValueError) as error:
        parser.error(str(error))
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "image-release.json").write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    (args.out / "image-tags.txt").write_text("\n".join(data["tags"]) + "\n", encoding="utf-8")
    (args.out / "release-notes.md").write_text(
        f"Webmister Download Manager, built from `{data['commit']}`.\n\n"
        + "\n".join(f"- `{tag}`" for tag in data["tags"])
        + "\n\nThis independent Web-focused fork is based on [Mister Download Manager](https://github.com/anefzaoui/misterdownloadmanager) by Ahmed Nefzaoui and contributors.\n\n"
        + "Downloads: Windows x86-64 and Linux x86-64 server bundles, Chromium ZIP and unsigned Firefox XPI. See SHA256SUMS.txt and RELEASE.json. Firefox signing/store publication is not performed. Native executables retain their MDM names; there is no fork auto-updater.\n\n"
        + "Container platform: linux/amd64. See image-release.json for the published digest. "
        + "Back up /config before upgrading; retain /downloads. Use a trusted LAN/VPN, not direct Internet exposure.\n",
        encoding="utf-8")
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        for key in ("image", "test_tag", "tag", "publish", "release"):
            value = str(data[key]).lower() if isinstance(data[key], bool) else data[key]
            output.write(f"{key}={value}\n")
        output.write("tags<<MDM_IMAGE_TAGS\n" + "\n".join(data["tags"]) + "\nMDM_IMAGE_TAGS\n")


if __name__ == "__main__":
    main()
