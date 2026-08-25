#!/usr/bin/env python3
"""Build Sindri's repository Markdown into the static documentation shell."""
from pathlib import Path
import html, re, sys
import markdown

ROOT=Path(__file__).resolve().parents[1]
OUT=Path(sys.argv[1]) if len(sys.argv)>1 else ROOT/'target/pages'
PAGES=[
 ('getting-started','Getting started','README.md','START HERE'),
 ('scripting','Decay scripting','docs/scripting.md','DEEP GUIDE'),
 ('language','Decay language reference','decay/LANGUAGE.md','REFERENCE'),
 ('cameras','Cameras','docs/cameras.md','ENGINE + EDITOR'),
 ('scenes','Scenes & components','docs/scene-format.md','CORE MODEL'),
 ('editor','Editor capabilities','docs/capabilities.md','EDITOR'),
 ('architecture','Architecture & direction','docs/architecture.md','INTERNALS'),
 ('features','Feature integration','docs/feature-integration-matrix.md','STATUS'),
]
# Some docs have evolved under different names. Keep the site build resilient while
# still failing if no sensible source exists.
FALLBACK={'scenes':['docs/scene-extraction.md'],'architecture':['docs/decay-direction.md']}
NAV=[('Learn',[('getting-started','Getting started'),('scenes','Scenes & components'),('cameras','Cameras'),('editor','Editor')]),('Decay',[('scripting','Scripting guide'),('language','Language reference')]),('Project',[('features','Feature integration'),('architecture','Architecture')])]

def source_for(slug,path):
 p=ROOT/path
 if p.exists(): return p
 for candidate in FALLBACK.get(slug,[]):
  q=ROOT/candidate
  if q.exists(): return q
 raise SystemExit(f'Missing documentation source for {slug}: {path}')

def links(text):
 # Repository-relative Markdown links become site routes when we publish that doc.
 replacements={'decay/LANGUAGE.md':'language/','docs/scripting.md':'scripting/','docs/cameras.md':'cameras/','docs/capabilities.md':'editor/','docs/feature-integration-matrix.md':'features/','README.md':'getting-started/'}
 for old,new in replacements.items():
  text=text.replace(f'href="{old}"',f'href="../{new}"')
  text=text.replace(f'href="../{old}"',f'href="../{new}"')
 return text

def nav(slug):
 chunks=[]
 for title,items in NAV:
  chunks.append(f'<div class="side-title">{title}</div>')
  chunks.extend(f'<a class="{"current" if s==slug else ""}" href="../{s}/">{label}</a>' for s,label in items)
 return ''.join(chunks)

def toc(body):
 found=re.findall(r'<h([23]) id="([^"]+)">(.+?)</h\1>',body)
 if not found:return ''
 return '<b>ON THIS PAGE</b>'+''.join(f'<a href="#{anchor}">{re.sub("<.*?>","",label)}</a>' for _,anchor,label in found[:16])

template='''<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="theme-color" content="#0b0d10"><title>{title} — Sindri Docs</title><link rel="stylesheet" href="../../assets/site.css"></head><body class="docs-body"><header class="nav"><a class="brand" href="../../"><span class="brandmark">S</span><span>Sindri</span><small>DOCS</small></a><nav><a href="../getting-started/">Docs</a><a href="../scripting/">Decay</a><a href="../../examples/gather/">Gather</a><a href="https://github.com/vardirhq/sindri2">GitHub ↗</a></nav></header><div class="docs-shell"><aside class="sidebar"><nav>{nav}</nav></aside><main class="doc"><div class="doc-kicker">{kicker}</div>{body}</main><aside class="toc">{toc}</aside></div><footer><span>SINDRI</span><p>Documentation generated from the repository sources.</p><a href="https://github.com/vardirhq/sindri2">Edit on GitHub ↗</a></footer></body></html>'''

md=markdown.Markdown(extensions=['fenced_code','tables','toc','sane_lists'])
for slug,title,path,kicker in PAGES:
 src=source_for(slug,path)
 raw=src.read_text()
 body=links(md.convert(raw)); md.reset()
 # README's remote logo is redundant inside the docs shell.
 body=re.sub(r'<p align="center">\s*<img.*?</p>','',body,count=1,flags=re.S)
 dest=OUT/'docs'/slug/'index.html';dest.parent.mkdir(parents=True,exist_ok=True)
 dest.write_text(template.format(title=html.escape(title),kicker=kicker,nav=nav(slug),body=body,toc=toc(body)))
print(f'Built {len(PAGES)} documentation pages')
