"""Seal the option-A swap contests (Yura, 2026-11-27). Metadata and hashes only.
Applies rule 5a (vendored dependency audits are not a prior audit of the same codebase) to
screen_optA.json, re-ranks by rule 8, then pins commits and sha256s. Bytes are hashed, never parsed or printed."""
import json,hashlib,subprocess,re,os,urllib.parse
HERE=os.path.dirname(os.path.abspath(__file__))
VENDORED=re.compile(r'(^|/)(lib|node_modules|dependencies|vendor)/',re.I)
def gh(args,binary=False):
    r=subprocess.run(['gh','api']+args,capture_output=True)
    if r.returncode!=0: raise SystemExit('gh api failed: '+' '.join(args)+' '+r.stderr.decode('utf-8','replace'))
    return r.stdout if binary else r.stdout.decode('utf-8').strip()
o=json.load(open(os.path.join(HERE,'screen_optA.json'),encoding='utf-8'))
rank=0; sealed=[]
for r in o:
    if 'pass_rank' not in r: continue
    own=[a for a in r['prior_audit_files'] if not VENDORED.search(a['path'])]
    rec={'slug':r['slug'],'order_sha256':r['order_sha256'],'screen_pass_rank':r['pass_rank'],
         'issues_at_screen':r['issues'],'labels_at_screen':r['label_count'],'duplicate_N_labels_at_screen':r['duplicate_N_labels'],
         'vendored_audit_paths_dropped':[a['path'] for a in r['prior_audit_files'] if VENDORED.search(a['path'])]}
    if not own: rec['reject']='rule 5a: only vendored dependency audits'; sealed.append(rec); continue
    rank+=1; rec['rank']=rank; rec['role']='selected' if rank<=4 else ('backup' if rank<=6 else 'qualifies, not selected')
    s=r['slug']; fr=f'code-423n4/{s}-findings'; cr=f'code-423n4/{s}'
    rec['findings']={'repo':fr,'commit':gh([f'repos/{fr}/commits/HEAD','-q','.sha'])}
    tree=gh([f'repos/{fr}/git/trees/{rec["findings"]["commit"]}','-q','[.tree[]|{path,sha}]|@json'])
    rec['findings']['tree_listing_sha256']=hashlib.sha256(tree.encode()).hexdigest()
    rec['findings']['report_md_blob']=next((t['sha'] for t in json.loads(tree) if t['path']=='report.md'),None)
    rec['codebase']={'repo':'https://github.com/'+cr,'commit':gh([f'repos/{cr}/commits/HEAD','-q','.sha']),'source':'c4-sponsor-snapshot HEAD at sealing'}
    pa=[]
    for a in own:
        b=gh(['-H','Accept: application/vnd.github.raw',f'repos/{cr}/contents/'+urllib.parse.quote(a['path'])+'?ref='+rec['codebase']['commit']],binary=True)
        pa.append({'path':a['path'],'git_blob':a['sha'],'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
    rec['prior_audit_files']=pa
    sealed.append(rec)
out={'version':1,'schema':'pairs-swap-sealed-v1','sealed_at':'2026-11-27 (Europe/Budapest)',
     'rule':'SELECTION.md rules 1-5, 5a, 7, 8 (rule 6 dropped: option A)','findings_read':False,'contests':sealed}
p=os.path.join(HERE,'sealed_optA.json')
open(p,'w',encoding='utf-8',newline='\n').write(json.dumps(out,indent=1)+'\n')
for r in sealed: print(r['slug'],r.get('role') or r['reject'],r.get('issues_at_screen'),[x['path'] for x in r.get('prior_audit_files',[])])
print('sealed_optA.json sha256',hashlib.sha256(open(p,'rb').read()).hexdigest())