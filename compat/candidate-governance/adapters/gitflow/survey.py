#!/usr/bin/env python3
"""Explicit read-only compatibility probe against an already fetched pinned provider.
Creates only a temporary fixture repository. Never runs apply/fetch/push/merge hooks.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

PIN = '310c43c5e3fe7effa19245dcc4970249e569004c'
parser = argparse.ArgumentParser()
parser.add_argument('--provider-root', type=Path, required=True)
parser.add_argument('--gitguard-bin', type=Path, required=True)
args = parser.parse_args()
provider = args.provider_root.resolve()
assert subprocess.check_output(['git', '-C', str(provider), 'rev-parse', 'HEAD']).decode().strip() == PIN
assert not subprocess.check_output(['git', '-C', str(provider), 'status', '--porcelain']), 'provider checkout must be clean'
with tempfile.TemporaryDirectory(prefix='gg-gitflow-survey-') as root:
    root = Path(root)
    env = dict(PATH='/usr/bin:/bin', HOME=str(root), LC_ALL='C', PYTHONDONTWRITEBYTECODE='1',
               GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1', GIT_OPTIONAL_LOCKS='0',
               GIT_AUTHOR_NAME='Fixture', GIT_AUTHOR_EMAIL='fixture@example.invalid',
               GIT_COMMITTER_NAME='Fixture', GIT_COMMITTER_EMAIL='fixture@example.invalid',
               GIT_AUTHOR_DATE='2000-01-01T00:00:00Z', GIT_COMMITTER_DATE='2000-01-01T00:00:00Z')
    def git(*argv):
        return subprocess.check_output(['git', '-C', str(root), *argv], env=env).decode().strip()
    git('init', '-q', '--object-format=sha1')
    (root / '.gitflow').mkdir()
    (root / '.gitflow/workflow.json').write_bytes((provider / 'profiles/classic-gitflow.json').read_bytes())
    (root / 'a').write_text('base\n')
    git('add', '.')
    git('commit', '-qm', 'base')
    base = git('rev-parse', 'HEAD')
    (root / 'a').write_text('candidate\n')
    git('add', 'a')
    git('commit', '-qm', 'feat: candidate')
    head = git('rev-parse', 'HEAD')
    def state():
        return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
                for p in root.rglob('*') if p.is_file()}
    before = state()
    records = []
    for case, native in [
        ('allow', ['check', str(root), '--base', base, '--head', head, '--source', 'feature/task', '--target', 'develop', '--json']),
        ('deny-target', ['check', str(root), '--base', base, '--head', head, '--source', 'feature/task', '--target', 'main', '--json']),
        ('unknown-object', ['check', str(root), '--base', base, '--head', '0'*40, '--source', 'feature/task', '--target', 'develop', '--json']),
        ('usage', ['check', str(root), '--unknown-flag']),
    ]:
        run = subprocess.run(['/usr/bin/python3', str(provider / 'scripts/gitflow.py'), *native], env=env, capture_output=True, timeout=30)
        records.append(dict(case=case, argv=[v.replace(str(root), '<fixture>') for v in native], exit=run.returncode,
                            stdout=json.loads(run.stdout), stderr=run.stderr.decode()))
    contract = dict(apiVersion='guard.partme.ai/v1alpha1', kind='GuardContract',
                    metadata=dict(id='gitguard.scope', revision='1'), spec=dict(rules=[dict(
                    id='scope.allowed-paths', description='Changed paths must remain in frozen task scope',
                    enforcement='enforce', assertion=dict(type='forbid_relation', subject='git-candidate', predicate='violates', object='task-scope'))]))
    policy_digest = hashlib.sha256(json.dumps(contract, separators=(',', ':')).encode()).hexdigest()
    request = dict(api_version='gitguard.check/v1alpha1', repo_root=str(root), repo_id='fixture-repo', candidate_oid=head,
                   scope=dict(task_id='task', requirement_ids=['R'], allowed_paths=[[97]], policy_digest=policy_digest, baseline_digest=None),
                   candidate=dict(worktree_id='w', base_oid=base, merge_group_id=None, members=[]), contract=contract)
    run = subprocess.run([str(args.gitguard_bin.resolve()), 'check'], input=json.dumps(request).encode(), env=env, capture_output=True, timeout=30)
    bundle = json.loads(run.stdout)
    assert before == state(), 'observation modified fixture files'
    print(json.dumps(dict(provider_commit=PIN, plugin_version=json.loads((provider/'plugin.json').read_text())['version'],
          candidate_oid=head, base_oid=base, native=records, gitguard=dict(exit=run.returncode, decision=bundle['envelope']['decision'],
          required_scopes=bundle['envelope']['coverage']['requiredScopes']), source_bytes_unchanged=True), indent=2, ensure_ascii=False))
