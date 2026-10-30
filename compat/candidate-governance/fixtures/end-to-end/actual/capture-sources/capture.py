#!/usr/bin/env python3
"""Known local fixture only. No installs, candidate plugins, external deps or generic supervisor."""
from pathlib import Path
import hashlib, json, os, shutil, subprocess, sys
HERE=Path(__file__).resolve().parent
OUT=Path(sys.argv[1]).resolve(); WORK=Path(sys.argv[2]).resolve(); WORK.mkdir(); REPO=WORK/'repository';shutil.copytree(HERE/'source',REPO)
FG=Path('/workspace/guard-implementation/flowguard/target/debug/examples/generate_approval_bridge')
TG=Path('/workspace/guard-implementation/testguard/target/debug/examples/generate_approval_bridge_tg')
NATIVE=Path('/workspace/guard-implementation-ledger/codeguard-ruff-native-capture/codeguard-f5661d5')
CG=Path('/workspace/guard-implementation-ledger/codeguard-ruff-cli-capture/codeguard')
RUFF=Path('/workspace/guard-toolchain/python-native/bin/ruff')
FIXED={str(NATIVE):'b7576afa78f5ab6fdc8133650a1c75dc113357ac927e553b4f5c67731eea3a8c',str(CG):'df8fe1a6e208db0bdc8f7fe6b443dbc826b7ce377a0568989ce5e37699940dba',str(RUFF):'f3eb080f0173e1a5884fabcc2c836f5a0ebea35fccf252e718fc71b17f821ef7'}
sha=lambda b:hashlib.sha256(b).hexdigest()
for path,digest in FIXED.items():assert sha(Path(path).read_bytes())==digest,path
ENV=os.environ.copy();ENV.update(CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='1',CARGO_TARGET_DIR='/workspace/guard-implementation/flowguard/target',GIT_CONFIG_NOSYSTEM='1',GIT_CONFIG_GLOBAL='/dev/null',GIT_AUTHOR_NAME='Fixture',GIT_AUTHOR_EMAIL='fixture@example.invalid',GIT_COMMITTER_NAME='Fixture',GIT_COMMITTER_EMAIL='fixture@example.invalid',GIT_AUTHOR_DATE='2026-10-09T00:00:00Z',GIT_COMMITTER_DATE='2026-10-09T00:00:00Z')
records=[]
def run(label,argv,expected=0):
 p=subprocess.run([str(x) for x in argv],cwd=REPO,env=ENV,capture_output=True,timeout=60)
 records.append({'label':label,'argv':[str(x) for x in argv],'cwd':str(REPO),'exit':p.returncode,'stdoutSha256':sha(p.stdout),'stderrSha256':sha(p.stderr)})
 assert p.returncode==expected,(label,p.returncode,p.stderr.decode(errors='replace'))
 return p
assert run('cargo-version',['cargo','--version']).stdout.decode().startswith('cargo 1.99.0 ')
run('cargo-lock',['cargo','generate-lockfile','--offline','--manifest-path',REPO/'Cargo.toml'])
run('git-init',['/usr/bin/git','-c','init.defaultBranch=main','init','-q'])
run('git-add',['/usr/bin/git','add','.']);run('git-commit',['/usr/bin/git','commit','-qm','trusted cross-guard fixture'])
BASE=run('git-base',['/usr/bin/git','rev-parse','HEAD']).stdout.decode().strip()
ENV['APPROVAL_BASE_OID']=BASE
OUT.mkdir(parents=True,exist_ok=True)
run('actual-sg-baseline',[FG,'sg-baseline',REPO,OUT])
(REPO/'specs/a.md').write_text((REPO/'specs/a.md').read_text().replace('original answer','reviewed answer'))
run('git-add-candidate',['/usr/bin/git','add','.']);run('git-commit-candidate',['/usr/bin/git','commit','-qm','reviewed current specification'])
OID=run('git-head',['/usr/bin/git','rev-parse','HEAD']).stdout.decode().strip()
OUT.mkdir(parents=True,exist_ok=True)
run('git-source-bundle',['/usr/bin/git','bundle','create',OUT/'source.bundle','HEAD'])
run('prepare-gg-ag',[FG,'prepare',REPO,OUT]);run('prepare-sg',[FG,'sg-prepare',REPO,OUT]);run('prepare-tg',[TG,'prepare',REPO,OUT])
base=json.loads((OUT/'binding.json').read_text()); cg=OUT/'codeguard';cg.mkdir(exist_ok=True)
source=sha((REPO/'app.py').read_bytes()); binding=dict(base,sourceSnapshotDigest='sha256:'+source)
argv=['lint','python',str(REPO),'--format=json','--file','app.py','--ruff-tool',str(RUFF)]
producer=FIXED[str(NATIVE)]
inv={'version':'codeguard.ruff-invocation/v1alpha1','packageVersion':'0.1.4','nativeSchema':'0.13.0','executable':str(NATIVE),'argv':argv,'runId':'controller-prepare-only','processExit':3,'producerSha256':producer}
ctx={'version':'codeguard.ruff-context/v1alpha1','runId':inv['runId'],'binding':binding,'target':'app.py','producerSha256':producer,'startedAt':'2026-10-09T00:00:00Z','finishedAt':'2026-10-09T00:00:01Z'}
contract={'apiVersion':'guard.partme.ai/v1alpha1','kind':'GuardContract','metadata':{'id':'ruff-f401','revision':'1'},'spec':{'rules':[{'id':'f401','enforcement':'enforce','assertion':{'type':'forbid_relation','subject':'python','predicate':'has','object':'unused-import'}}]}}
mapping={'version':'codeguard.mapping/v1alpha1','entries':[{'source':{'kind':'finding','tool_id':'ruff','native_rule_id':'F401'},'rule_id':'f401'},{'source':{'kind':'gap','detail':'ruff_f401_scope_incomplete'},'rule_id':'f401'}]}
def write(name,data):(cg/name).write_text(json.dumps(data,separators=(',',':')))
for name,data in [('prepare-invocation.json',inv),('prepare-context.json',ctx),('contract-input.json',contract),('mapping.json',mapping)]:write(name,data)
def sidecar(invfile,ctxfile,nativefile):return [CG,'guard-project-ruff','--invocation',cg/invfile,'--native-report',cg/nativefile,'--context',cg/ctxfile,'--mapping',cg/'mapping.json','--contract',cg/'contract-input.json']
assert not (cg/'not-yet-executed.json').exists()
p=run('cg-prepare-before-native',sidecar('prepare-invocation.json','prepare-context.json','not-yet-executed.json'),4)
(cg/'prepare.stdout.json').write_bytes(p.stdout);(cg/'prepare.stderr.json').write_bytes(p.stderr)
assert json.loads(p.stdout)['envelope']['decision'] is None
run('freeze-cg-expectation',[FG,'cg-prepare',REPO,OUT]);run('freeze-fg-before-results',[FG,'freeze',REPO,OUT])
# Save controller expectations before any native execution or completed specialist result.
protected={str(p.relative_to(OUT)):sha(p.read_bytes()) for p in OUT.rglob('*.json')}
p=run('actual-native-codeguard',[NATIVE,*argv],3);(cg/'native.json').write_bytes(p.stdout);(cg/'native.stderr').write_bytes(p.stderr)
native=json.loads(p.stdout);inv=dict(inv,runId=native['run_id']);ctx=dict(ctx,runId=native['run_id'])
write('invocation.json',inv);write('context.json',ctx)
p=run('actual-cg-projection',sidecar('invocation.json','context.json','native.json'));(cg/'bundle.json').write_bytes(p.stdout);(cg/'projection.stderr').write_bytes(p.stderr)
bundle=json.loads(p.stdout);assert bundle['nativeExit']==3;assert bundle['artifacts']['domain'].encode()==(cg/'native.json').read_bytes()
write('envelope.json',bundle['envelope'])
for name in ['contract','facts','report','domain']:(cg/(name+'.json')).write_bytes(bundle['artifacts'][name].encode())
assert (cg/'contract.json').read_bytes()==(cg/'prepared-contract.json').read_bytes()
assert bundle['envelope']['coverage']['requiredScopes']==json.loads((cg/'expected.json').read_text())['required_scopes']
tg=OUT/'testguard'; command=['cargo','test','--locked','--offline','--manifest-path',str(REPO/'Cargo.toml'),'-p','cross-guard-app','--lib','tests::required_answer','--']
p=run('actual-cargo-discovery',command+['--list','--format','terse']);(tg/'inventory.raw').write_bytes(p.stdout);(tg/'inventory.stderr').write_bytes(p.stderr)
p=run('actual-cargo-run',command+['--format','pretty','--test-threads=1']);(tg/'stdout.raw').write_bytes(p.stdout);(tg/'stderr.raw').write_bytes(p.stderr)
(tg/'capture.json').write_text(json.dumps({'exit_code':p.returncode,'interrupted':False,'profile':'trusted fixed fixture only','command':command+['--format','pretty','--test-threads=1']},indent=2)+'\n')
run('actual-tg-producer',[TG,'finish',REPO,OUT]);run('actual-sg-producer',[FG,'sg-finish',REPO,OUT]);run('actual-ag-gg-producers',[FG,'finish',REPO,OUT])
assert not run('git-status-after',['/usr/bin/git','status','--porcelain']).stdout
for path,digest in protected.items():assert sha((OUT/path).read_bytes())==digest,('controller input mutated',path)
for name in ['archguard','codeguard','testguard','gitguard']:
 e=json.loads((OUT/name/'envelope.json').read_text());assert e['binding']['candidateOid']==OID;assert e['binding']['baseOid']==BASE;assert e['decision']=='ALLOW';assert e['coverage']['status']=='complete'
manifest={'version':'flowguard.actual-providers/v1','authority':'fixture-ci-only; no production issuer','candidate':OID,'base':BASE,'repository':'cross-guard-repo','producerPins':{'specguard':'4560d133a1ac28dc50fff5aad7aa76062cdc6911','archguard':'7f21806f2a0af544df978d0bfea3e1a840091f2b','gitguard':'ff1afb3164ab5720c3964cfcc3add542cb8b8831','testguard':'e2e9147117944a2923373705554d5bdfb5c88ee8','codeguardProjection':'72d80ae453c0380ad7061c10847595c5a4ac1736','codeguardNative':'f5661d51dd1b2629f3b87b726fb09a9060ce7958','guardengineRust':'20c53cdbff385034afd750fdebd38ae7d0fab071','guardengineCodeguard':'c80ec325449842d51007db2fd507040c53dc8f51'},'protectedBeforeResults':protected,'commands':records,'binarySha256':dict(FIXED,**{str(FG):sha(FG.read_bytes()),str(TG):sha(TG.read_bytes())}),'generatorSha256':{p.name:sha(p.read_bytes()) for p in [HERE/'capture.py',HERE/'generate_testguard.rs',HERE.parents[2]/'examples/generate_approval_bridge.rs']},'artifacts':{str(p.relative_to(OUT)):sha(p.read_bytes()) for p in OUT.rglob('*') if p.is_file() and p.name!='PROVENANCE.json'}}
(OUT/'PROVENANCE.json').write_text(json.dumps(manifest,indent=2)+'\n')
print('actual five producer complete/ALLOW artifacts with protected expectations and native bytes retained',OID)
