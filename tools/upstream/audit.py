"""Report upstream overlaps without merging, staging, switching branches or creating commits.

--check needs no upstream ref/history (CI/shallow clones). --preview-worktree uses
per-file merge-file in temporary files, NOT a full rename-aware merge simulation.
Only paths/status/counts are printed; never source contents or runtime state.
"""
import argparse
from contextlib import nullcontext
import fnmatch
import json
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = Path(__file__).with_name("boundaries.json")


def git(root, *args, allowed=(0,)):
    result = subprocess.run(["git", "-c", "core.quotepath=false", *args], cwd=root, capture_output=True)
    if result.returncode not in allowed:
        raise RuntimeError(f"git {args[0]} failed: {result.stderr.decode('utf-8', errors='replace').strip()}")
    return result


def stats(root, *revisions):
    raw = git(root, "diff", "--no-ext-diff", "--no-renames", "--numstat", "-z", *revisions, "--").stdout
    out = {}
    for row in raw.split(b"\0"):
        if not row:
            continue
        added, removed, path = row.decode("utf-8").split("\t", 2)
        out[path] = {"added": int(added) if added != "-" else None,
                     "removed": int(removed) if removed != "-" else None}
    return out


def ownership(path, manifest):
    for key, label in [("generated", "generated"), ("fork_only", "fork-only"), ("overlays", "overlay")]:
        if any(fnmatch.fnmatchcase(path, pattern) for pattern in manifest.get(key, [])):
            return label
    return "bridge" if path in manifest.get("bridges", {}) else "review"


def check(root, manifest):
    errors = []
    for item in manifest["checks"]:
        path = root / item["path"]
        if not path.is_file():
            errors.append(f"missing {item['path']}")
            continue
        source = path.read_text(encoding="utf-8")
        for pattern in item.get("required", []):
            if not re.search(pattern, source):
                errors.append(f"{item['path']}: missing hook {pattern}")
        for pattern in item.get("forbidden", []):
            if re.search(pattern, source):
                errors.append(f"{item['path']}: implementation leaked across boundary ({pattern})")
    return errors


def blob(root, revision, path):
    # A missing blob is a structural add/delete, not an empty base for an invented merge.
    exists = git(root, "cat-file", "-e", f"{revision}:{path}", allowed=(0, 1, 128))
    return git(root, "show", f"{revision}:{path}").stdout if exists.returncode == 0 else None


def preview(root, base, upstream, path, directory):
    local = root / path
    if local.is_symlink():
        return "manual: symlink"
    ours = local.read_bytes() if local.is_file() else None
    before, theirs = blob(root, base, path), blob(root, upstream, path)
    if ours is None or before is None or theirs is None:
        return "manual: add/delete/type change"
    contents = [ours, before, theirs]
    if any(b"\0" in value for value in contents):
        return "manual: binary"
    # Source-tree text may have checkout CRLF. This is only a text-hunk estimate;
    # custom merge/clean filters, renames, modes and directory conflicts need a real merge.
    files = [directory / name for name in ("ours", "base", "theirs")]
    for file, value in zip(files, contents):
        file.write_bytes(value.replace(b"\r\n", b"\n"))
    result = git(root, "merge-file", "--stdout", *map(str, files), allowed=tuple(range(128)))
    return "text clean" if result.returncode == 0 else "text conflict"


def report(root, upstream, manifest, with_preview=False):
    head = git(root, "rev-parse", "HEAD").stdout.decode().strip()
    tip = git(root, "rev-parse", "--verify", f"{upstream}^{{commit}}").stdout.decode().strip()
    base = git(root, "merge-base", head, tip).stdout.decode().strip()
    # These are current tracked worktree contents versus the common ancestor, NOT HEAD only.
    local, incoming = stats(root, base), stats(root, base, tip)
    untracked = git(root, "ls-files", "--others", "--exclude-standard", "-z").stdout.decode().split("\0")
    for path in filter(None, untracked):
        local[path] = {"added": None, "removed": None, "untracked": True}
    counts = list(map(int, git(root, "rev-list", "--left-right", "--count", f"{head}...{tip}").stdout.split()))
    overlaps = []
    # Temporary files only; never the repository index or a synthetic commit.
    temp_root = root / "target" / "upstream-audit"
    if with_preview:
        temp_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="preview-", dir=temp_root) if with_preview else nullcontext() as folder:
        for path in sorted(local.keys() & incoming.keys()):
            kind = ownership(path, manifest)
            overlaps.append({"path": path, "ownership": kind,
                             "fork": local[path], "upstream": incoming[path],
                             "preview": preview(root, base, tip, path, Path(folder)) if with_preview else "not requested",
                             "note": manifest.get("bridges", {}).get(path, "Review upstream ownership collision" if kind == "fork-only" else "")})
    return {"head": head, "upstream": tip, "merge_base": base,
            "commits": {"fork": counts[0], "upstream": counts[1]},
            "dirty": bool(git(root, "status", "--porcelain").stdout),
            "local_files": len(local), "upstream_files": len(incoming), "overlaps": overlaps,
            "scope": "Current worktree. Per-file text preview only; not a full Git merge or semantic compatibility proof."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="enforce adapter hooks; no upstream history required")
    parser.add_argument("--upstream", default="upstream/main", help="already-fetched ref (never fetched automatically)")
    parser.add_argument("--preview-worktree", action="store_true", help="estimate overlapping text conflicts, including uncommitted files")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
    try:
        if args.check:
            errors = check(ROOT, manifest)
            print("\n".join(errors) if errors else "Fork integration boundaries: OK (structural checks, not behavioral tests)")
            return bool(errors)
        result = report(ROOT, args.upstream, manifest, args.preview_worktree)
    except (RuntimeError, OSError) as error:
        parser.exit(2, f"{error}\n")
    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print(f"HEAD {result['head'][:12]} / upstream {result['upstream'][:12]} / base {result['merge_base'][:12]}")
        print(f"Commits: {result['commits']['fork']} fork / {result['commits']['upstream']} upstream; dirty: {result['dirty']}")
        print(f"Changed files: {result['local_files']} local (includes untracked) / {result['upstream_files']} upstream; overlaps: {len(result['overlaps'])}")
        for item in result["overlaps"]:
            print(f"  {item['ownership']:10} {item['preview']:35} {item['path']}")
        print(result["scope"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
