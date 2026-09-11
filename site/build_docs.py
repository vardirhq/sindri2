#!/usr/bin/env python3
"""Build Sindri's repository Markdown into the static documentation shell."""

from pathlib import Path, PurePosixPath
from urllib.parse import quote
import html
import posixpath
import re
import sys

import markdown


ROOT = Path(__file__).resolve().parents[1]
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "target/pages"
REPOSITORY = "https://github.com/vardirhq/sindri2/blob/main"

PAGES = [
    ("documentation", "Documentation map", "docs/README.md", "ALL DOCS"),
    ("getting-started", "Getting started", "README.md", "START HERE"),
    ("project", "Projects", "docs/project-format.md", "PROJECT MODEL"),
    ("scenes", "Scenes & components", "docs/scene-extraction.md", "CORE MODEL"),
    ("cameras", "Cameras", "docs/cameras.md", "ENGINE + EDITOR"),
    ("physics", "2D physics", "docs/physics.md", "ENGINE + DECAY"),
    ("editor", "Editor capabilities", "docs/capabilities.md", "EDITOR"),
    ("scripting", "Decay scripting", "docs/scripting.md", "DEEP GUIDE"),
    ("language", "Decay language reference", "decay/LANGUAGE.md", "REFERENCE"),
    ("weave", "Weave responsive UI", "docs/weave.md", "UI STYLING"),
    ("weave-reference", "Weave reference", "docs/weave-reference.md", "REFERENCE"),
    ("export", "Web export", "docs/export.md", "SHIPPING"),
    ("parity", "Engine parity", "docs/parity.md", "STATUS"),
    ("architecture", "Architecture & direction", "docs/FEASIBILITY.md", "INTERNALS"),
]

NAV = [
    (
        "Learn",
        [
            ("documentation", "Documentation map"),
            ("getting-started", "Getting started"),
            ("project", "Projects"),
            ("scenes", "Scenes & components"),
            ("cameras", "Cameras"),
            ("physics", "2D physics"),
            ("editor", "Editor"),
        ],
    ),
    (
        "Languages",
        [
            ("scripting", "Decay scripting"),
            ("language", "Decay reference"),
            ("weave", "Weave guide"),
            ("weave-reference", "Weave reference"),
        ],
    ),
    (
        "Ship",
        [
            ("export", "Web export"),
            ("features", "Feature integration"),
            ("architecture", "Architecture"),
        ],
    ),
]

PAGE_BY_SOURCE = {path: slug for slug, _, path, _ in PAGES}


def source_for(path: str) -> Path:
    source = ROOT / path
    if not source.is_file():
        raise SystemExit(f"Missing documentation source: {path}")
    return source


def documentation_links(rendered: str, source_path: str) -> str:
    """Route published docs locally and repository docs to their source."""

    source_directory = str(PurePosixPath(source_path).parent)

    def replace(match: re.Match[str]) -> str:
        href = match.group(1)
        if href.startswith(("http://", "https://", "mailto:", "#")):
            return match.group(0)

        target, separator, anchor = href.partition("#")
        resolved = posixpath.normpath(posixpath.join(source_directory, target))
        if resolved == ".":
            resolved = source_path

        if resolved in PAGE_BY_SOURCE:
            destination = f"../{PAGE_BY_SOURCE[resolved]}/"
            if separator:
                destination += f"#{anchor}"
            return f'href="{destination}"'

        if (ROOT / resolved).is_file():
            destination = f"{REPOSITORY}/{quote(resolved)}"
            if separator:
                destination += f"#{anchor}"
            return f'href="{destination}"'

        return match.group(0)

    return re.sub(r'href="([^"]+)"', replace, rendered)


def navigation(slug: str) -> str:
    chunks = []
    for title, items in NAV:
        chunks.append(f'<div class="side-title">{title}</div>')
        chunks.extend(
            f'<a class="{"current" if item_slug == slug else ""}" '
            f'href="../{item_slug}/">{label}</a>'
            for item_slug, label in items
        )
    return "".join(chunks)


def table_of_contents(body: str) -> str:
    found = re.findall(r'<h([23]) id="([^"]+)">(.+?)</h\1>', body)
    if not found:
        return ""
    items = "".join(
        f'<a href="#{anchor}">{re.sub("<.*?>", "", label)}</a>'
        for _, anchor, label in found[:16]
    )
    return f"<b>ON THIS PAGE</b>{items}"


# The lockup is inline SVG rather than an asset so a documentation page is a
# single request against one stylesheet, and so it cannot go missing from a
# route that was assembled without the images.
MARK = (
    '<svg width="26" height="26" viewBox="0 0 32 32" fill="none" aria-hidden="true">'
    '<polygon points="16,3 27.26,9.5 27.26,22.5 16,29 4.74,22.5 4.74,9.5" '
    'stroke="#e6e1d4" stroke-width="1.6" fill="none"/>'
    '<polygon points="16,9 22.06,12.5 22.06,19.5 16,23 9.94,19.5 9.94,12.5" '
    'stroke="#e6e1d4" stroke-width="1.2" fill="none"/>'
    '<polygon points="16,13.2 18.42,14.6 18.42,17.4 16,18.8 13.58,17.4 13.58,14.6" '
    'fill="#f0c050"/>'
    "</svg>"
)

TEMPLATE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="theme-color" content="#0d1117">
<title>{title} — Sindri Engine docs</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Pixelify+Sans:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet">
<link rel="stylesheet" href="../../assets/site.css">
</head>
<body class="docs-body">
<header class="nav">
<div class="shell">
<a class="brand" href="../../">{mark}<span>
<span class="wordmark">S<i>I</i>NDR<i>I</i></span>
<span class="tagline"><span class="rule"></span><span class="chip">DOCS</span></span>
</span></a>
<nav>
<a href="../documentation/">docs</a>
<a href="../scripting/">decay</a>
<a href="../weave/">weave</a>
<a href="../../examples/gather/">gather</a>
<a class="ghost" href="https://github.com/vardirhq/sindri2">github ↗</a>
</nav>
</div>
</header>
<div class="docs-shell">
<aside class="sidebar"><nav>{nav}</nav></aside>
<main class="doc"><div class="doc-kicker">{kicker}</div>{body}</main>
<aside class="toc">{toc}</aside>
</div>
<footer>
<div class="shell">
<div class="brand">{mark}<span>
<span class="wordmark">SINDRI</span>
<span class="tagline"><span class="rule"></span><span class="chip">ENGINE</span></span>
</span></div>
<span>generated from the repository sources</span>
<a href="https://github.com/vardirhq/sindri2">edit on github ↗</a>
</div>
</footer>
</body>
</html>
"""

renderer = markdown.Markdown(extensions=["fenced_code", "tables", "toc", "sane_lists"])
for slug, title, path, kicker in PAGES:
    source = source_for(path)
    body = documentation_links(renderer.convert(source.read_text()), path)
    renderer.reset()

    # README's remote logo is redundant inside the documentation shell.
    body = re.sub(
        r'<p align="center">\s*<img.*?</p>',
        "",
        body,
        count=1,
        flags=re.S,
    )
    destination = OUT / "docs" / slug / "index.html"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(
        TEMPLATE.format(
            title=html.escape(title),
            kicker=kicker,
            mark=MARK,
            nav=navigation(slug),
            body=body,
            toc=table_of_contents(body),
        )
    )

print(f"Built {len(PAGES)} documentation pages")
