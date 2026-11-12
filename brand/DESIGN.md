---
name: Webmister Download Manager
description: A narrow fork identity and GitHub-native documentation extension for the existing MDM Web workspace.
colors:
  navy: "#123565"
  white: "#ffffff"
  mint: "#69df91"
rounded:
  logo-tile: "13px"
components:
  fork-mark:
    backgroundColor: "{colors.navy}"
    rounded: "{rounded.logo-tile}"
    width: "64px"
    height: "64px"
---

# Design System: Webmister Download Manager

## Overview

**Creative North Star: "Documentation first"**

Webmister Download Manager is an independent, Web-focused fork of Mister Download Manager. The original Mister Download Manager was created by Ahmed Nefzaoui and contributors. Webmister's compact authored mark distinguishes the fork while readable documentation shows the actual browser workspace. The name and mark do not imply upstream endorsement or an official MDM release.

This is an approved narrow branding extension, not a replacement visual world. GitHub owns README reading typography; the existing Web workspace remains governed by [its incumbent design reference](../crates/remote/web/DESIGN.md), with only its fork name/logo rule extended. The root DESIGN.md describes the upstream marketing site, not this brand. Engine executable names, data paths/formats and protocol IDs remain compatible; original desktop artwork and source attribution remain intact.

**Key Characteristics:**
- Exact fork name, original mark and explicit upstream credit.
- GitHub-native reading structure, not a marketing-site reskin.
- Real shipped controls with clearly labeled synthetic demonstration data.
- Reproducible assets with source provenance, not performance or release claims.

Authority: the approved [surface contract](../.impeccable/surfaces/readme-md.md), [brand brief](brief.md), [SVG source](webmister.svg) and [README](../README.md). This document and its local [.impeccable/design.json](.impeccable/design.json) cover this brand only. They neither supersede the app system nor repair inherited palette/radius drift. The supplied finish review is **SHIP**, scoped to README, identity, presentation and documentation truth, with no material findings; it is not production-runtime or release acceptance.

## Colors

The three frontmatter colors are the mark's complete authored palette, not application theme tokens.

### Primary
- **Deep Navy** (`navy`): the tile beneath both strokes, keeping the mark self-contained on light and dark document surfaces.

### Secondary
- **Download Mint** (`mint`): the downward arrow and baseline; an identity accent, not a new application success or button color.

### Neutral
- **Letter White** (`white`): the W stroke. The SVG's short white notation is equivalent to the normalized frontmatter value.

**The Local Identity Rule.** Apply these tokens to the fork mark, not to inherited workspace controls or the upstream marketing site.

## Typography

README headings, prose, links, tables, summaries and fenced code use GitHub's native renderer. No fork display font, fixed reading-size scale or custom tracking is established here; therefore no typography tokens are invented. Keep the exact product name in live text beside the mark. SVG title/description and image alt text provide an accessible identity without turning prose into artwork.

## Layout

Use a readable document hierarchy: product heading, brief attributed fork statement, descriptive section links, evidence, then numbered setup instructions with selectable code. Preserve native headings, lists and tables rather than composing a simulated application or promotional card grid. The approved first-viewport composition belongs to the surface contract, not a new app layout system.

The README displays the square mark at (128 × 128 px) beneath the product heading, then a centered tagline and fork credit. Grey, unlinked Windows/Linux/Docker and Firefox/Chrome badges communicate pending publication; live star links target this fork. GitHub-native navigation, benefit copy and feature tables follow the full-width workspace image (1100 px width attribute). Its `picture` element selects dark-theme workspace evidence with a light fallback.

Twelve Web UI images appear in a six-row, two-column HTML table with top-aligned cells and descriptive captions. The mobile capture has a 260 px width attribute; other grid images fit their cells rather than imposing a fixed detail-image width. Captions explicitly explain post-add torrent selection and horizontal scrolling on narrow screens. The user requested upstream's README structure, not a replacement app or site design; no custom fonts, CSS or motion ship in the README.

## Elevation & Depth

The authored mark is flat: solid tile, two strokes, no gradient, shadow or glow. README depth and disclosure behavior belong to GitHub; captured dialog elevation belongs to the existing application. This brand establishes no shadow or motion tokens.

## Shapes

The mark uses a square SVG viewBox (64 × 64). The frontmatter tile radius describes that natural coordinate size, not a reusable control radius; scale the complete SVG proportionally. The W uses a stroke width of (6 SVG units); the arrow and baseline use (4.5 SVG units). Both use rounded caps and joins. Preserve the source path geometry rather than recreating the W as a font glyph.

## Components

### Fork mark

The sole brand component is [webmister.svg](webmister.svg): a white W beneath a mint downward arrow, with a mint baseline on navy. It includes an accessible title and description. This is a static identity asset, not a button; it defines no hover, pressed or focus variant. The sidecar renders an inline copy of the authored SVG only, not synthesized app primitives.

The SVG is newly authored fork artwork, not a recolored upstream logo or third-party download icon. [assets.json](assets.json) records its GPL-3.0-or-later license, LF-normalized source SHA-256, method and generated files. `node tools/web/brand-assets.mjs` rasterizes it without retouching through Chromium/Playwright at (16, 32, 48, 128 and 256 px) into `extension/icons/webmister-*.png`; the same fork identity is used by the Web UI and extension. Retain upstream application, engine, translations and desktop-artwork credits in [ATTRIBUTION.md](../ATTRIBUTION.md) and source notices.

### Screenshot evidence

[captures.json](../docs/images/webmister/captures.json) inventories twelve captures: light/dark workspace, download details, torrent files, mobile workspace, add URL, add torrent, connection settings, authentication settings, devices, queues and categories. `node tools/web/readme-shots.mjs` captures the shipped HTML/CSS/JS against the public `tools/web/preview.mjs` fixture using Chromium/Playwright. Desktop viewports are 1440 × 940 px, extended to 1440 × 1600 px for management controls; mobile is 390 × 844 px. Dialog/form/fieldset screenshots are element captures, not full-viewport exports. Viewports, origins and source hashes are recorded in the manifest; shipping PNGs also carry embedded source-origin metadata.

**The Evidence Caption Rule.** Keep synthetic demonstration data explicitly labeled in images and adjacent captions; screenshots are neither live transfers nor speed benchmarks, and must never expose private account or server data.

README captions distinguish post-add torrent selection from a pre-start picker. Preserve that distinction and the release-availability caveat: prepared publication machinery is not evidence of an already downloadable release. The captures were regenerated for this README pass; doing so neither validates the server release nor publishes artifacts.

**The Benchmark Attribution Rule.** The owner approved an upstream benchmark section with the original chart, selected report medians and source revision/date. Label those as upstream MDM results, not measurements of Webmister, its current engine, containers or remote deployments. Never promote them into the fork's hero speed claim.

## Do's and Don'ts

### Do:
- **Do** use the exact name Webmister Download Manager and credit Ahmed Nefzaoui and upstream contributors.
- **Do** preserve the authored SVG geometry and regenerate PNGs from its recorded source.
- **Do** retain GitHub-native reading structure, descriptive alt text and selectable setup commands.
- **Do** keep demo-data captions, source provenance and release-availability caveats visible.
- **Do** preserve engine names, data compatibility, protocol IDs and upstream artwork attribution.

### Don't:
- **Don't** present the fork as official MDM or imply upstream endorsement, signing or store publication.
- **Don't** apply logo colors or tile corners as a replacement app palette or radius scale.
- **Don't** invent typography, app components, performance claims or adoption evidence for this brand.
- **Don't** retouch screenshots, conceal fixture data or publish private instance content.
- **Don't** treat the scoped SHIP review as proof of runtime hardening or successful release publication.
