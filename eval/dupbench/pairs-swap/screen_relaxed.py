"""Metadata-only screen for the pairs swap (rule in P2R2_PROGRESS 'pairs SWAP selection rule').
Reads names only: repo trees (paths), label names, issue counts via search total_count. No content."""
import json,hashlib,subprocess,re,sys
SALT='p2r2-pairs-swap-2026-11-27|'
USED={'party','wildcat','pooltogether','tapioca','maia','panoptic'}
import os as _os
HERE=_os.path.dirname(_os.path.abspath(__file__))
DB=_os.path.dirname(HERE)
man=json.load(open(_os.path.join(DB,'manifest.json'),encoding='utf-8'))['contests']
dup_suffix={c['slug'][8:].lower() for c in man}
pairs_suf={p['contest']['slug'][8:].lower() for p in json.load(open(_os.path.join(DB,'pairs.json')))['pairs']}
tok=lambda x: x.split('-')[0]
dup_tok={tok(x) for x in dup_suffix|pairs_suf}
cand=[s for s in json.load(open(_os.path.join(HERE,'cand0.json'))) if '2022-08'<=s[:7]<='2024-12' and 'mitigation' not in s]
cand.sort(key=lambda s: hashlib.sha256((SALT+s).encode()).hexdigest())
def api(path, q=None):
    a=['gh','api',path]+(['-q',q] if q else [])
    r=subprocess.run(a,capture_output=True,text=True,encoding='utf-8')
    return r.returncode, r.stdout
out=[]; picked=0
for s in cand:
    rec={'slug':s,'order_sha256':hashlib.sha256((SALT+s).encode()).hexdigest()}
    suf=s[8:].lower()
    if any(u in suf for u in USED): rec['reject']='sponsor used before'; out.append(rec); continue
    if suf in dup_suffix or tok(suf) in dup_tok: rec['reject']='sponsor token in Dup-Bench or pairs'; out.append(rec); continue
    rc,t=api(f'repos/code-423n4/{s}-findings/git/trees/HEAD','[.tree[].path]|@json')
    paths=json.loads(t) if rc==0 and t.strip() else []
    rec['findings_has_report_md']='report.md' in paths
    if not rec['findings_has_report_md']: rec['reject']='no report.md'; out.append(rec); continue
    r=subprocess.run(['gh','api','--paginate',f'repos/code-423n4/{s}-findings/labels?per_page=100','-q','.[].name'],capture_output=True,text=True,encoding='utf-8')
    labels=[l for l in r.stdout.splitlines() if l] if r.returncode==0 else None
    if labels is None: rec['reject']='label API error'; out.append(rec); continue
    rec['label_count']=len(labels)
    ndup=sum(1 for l in labels if re.fullmatch(r'duplicate-\d+',l))
    rec['labels_selected_for_report']='selected for report' in labels; rec['duplicate_N_labels']=ndup
    if not rec['labels_selected_for_report'] or ndup==0: rec['reject']='no judge grouping labels'; out.append(rec); continue
    rc,t=api(f'search/issues?q=repo:code-423n4/{s}-findings+is:issue&per_page=1','.total_count')
    rec['issues']=int(t.strip()) if rc==0 and t.strip().isdigit() else None
    if rec['issues'] is None or rec['issues']>500: rec['reject']='issue count missing or >500'; out.append(rec); continue
    rc,t=api(f'repos/code-423n4/{s}/git/trees/HEAD?recursive=1','[.tree[]|select(.type=="blob")|{path,sha}]|@json')
    blobs=json.loads(t) if rc==0 and t.strip() else []
    aud=[b for b in blobs if re.search(r'audit',b['path'],re.I) and re.search(r'\.(pdf|md)$',b['path'],re.I) and not b['path'].lower().endswith('readme.md')]
    rec['prior_audit_files']=aud[:10]
    if not aud: rec['reject']='no prior audit file in contest repo'; out.append(rec); continue
    rec['accepted']=picked+1; picked+=1; out.append(rec)
    if picked>=6: break
json.dump(out,open(_os.path.join(HERE,'screen_relaxed.json'),'w',encoding='utf-8'),indent=1)
for r in out: print(r['slug'], r.get('accepted') or r.get('reject'), r.get('issues',''), len(r.get('prior_audit_files',[])))