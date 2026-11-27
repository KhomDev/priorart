import json,re,os,subprocess,sys
import os as _os
HERE=_os.path.dirname(_os.path.abspath(__file__))
DB=_os.path.dirname(HERE)
rows=[l.rstrip('\n').split('\t') for l in open(_os.path.join(HERE,'code-423n4-repos.tsv'),encoding='utf-8')]
names={r[0] for r in rows}
srcl=json.load(open(_os.path.join(DB,'manifest.json'),encoding='utf-8'))['contests']
dup={c['slug'] for c in srcl}
pairs={p['contest']['slug'] for p in json.load(open(_os.path.join(DB,'pairs.json')))['pairs']}
cand=[]
for n in sorted(names):
    m=re.match(r'^(\d{4})-(\d{2})-(.+)-findings$',n)
    if not m: continue
    slug=n[:-len('-findings')]
    if slug in dup or slug in pairs: continue
    if slug not in names: continue  # need the contest (code) repo
    cand.append(slug)
print('findings repos with code repo, not in Dup-Bench/pairs:',len(cand))
json.dump(cand,open(_os.path.join(HERE,'cand0.json'),'w'),indent=0)