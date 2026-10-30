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
import sys
import os
import signal

PIN = '310c43c5e3fe7effa19245dcc4970249e569004c'
parser = argparse.ArgumentParser()
parser.add_argument('--provider-root', type=Path, required=True)
parser.add_argument('--gitguard-bin', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
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
    (root / '.git/invalid-message').write_bytes(b'\xff')
    before = state()
    # Independently compute protected expected normalized policy before collecting reports.
    sys.dont_write_bytecode = True
    sys.path.insert(0, str(provider / 'scripts'))
    from gitflow.policy import validate, digest
    from gitflow.organization import resolve
    expected_native_hash = digest(resolve(validate(json.loads((root/'.gitflow/workflow.json').read_bytes())), reader=lambda name: subprocess.check_output(['git','-C',str(root),'show',base+':'+name],env=env)))
    records = []
    for case, native in [
        ('allow', ['check', str(root), '--base', base, '--head', head, '--source', 'feature/task', '--target', 'develop', '--json']),
        ('deny-target', ['check', str(root), '--base', base, '--head', head, '--source', 'feature/task', '--target', 'main', '--json']),
        ('unknown-object', ['check', str(root), '--base', base, '--head', '0'*40, '--source', 'feature/task', '--target', 'develop', '--json']),
        ('usage', ['check', str(root), '--unknown-flag']),
        ('internal-error', ['check', str(root), '--base', base, '--head', head, '--source', 'feature/task', '--target', 'develop', '--message-file', str(root/'.git/invalid-message'), '--json']),
    ]:
        run = subprocess.run(['/usr/bin/python3', str(provider / 'scripts/gitflow.py'), *native], env=env, capture_output=True, timeout=30)
        (args.output / (case + '.stdout')).write_bytes(run.stdout)
        (args.output / (case + '.stderr')).write_bytes(run.stderr)
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
    # Explicit transport fault: pause the launcher before provider execution, then enforce deadline.
    # This proves timeout classification/cleanup, not a naturally slow native result.
    stopped = subprocess.Popen(['/usr/bin/python3', '-c', 'import os,signal; os.kill(os.getpid(),signal.SIGSTOP)'], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    try:
        stopped.communicate(timeout=0.1)
        raise AssertionError('transport fault did not time out')
    except subprocess.TimeoutExpired:
        os.killpg(stopped.pid, signal.SIGKILL)
        timeout_stdout, timeout_stderr = stopped.communicate(timeout=5)
    (args.output/'transport-timeout.json').write_text(json.dumps(dict(kind='consumer-transport-injection', native_executed=False, deadline_seconds=0.1, exit=stopped.returncode, reaped=True, stdout_hex=timeout_stdout.hex(),stderr_hex=timeout_stderr.hex()))+'\n')
    comparisons = []
    for allowed in [[97], [98]]:
        request['scope']['allowed_paths'] = [allowed]
        run = subprocess.run([str(args.gitguard_bin.resolve()), 'check'], input=json.dumps(request).encode(), env=env, capture_output=True, timeout=30)
        bundle = json.loads(run.stdout)
        label = 'gg-path-' + bytes(allowed).decode()
        (args.output / (label + '.stdout')).write_bytes(run.stdout)
        comparisons.append(dict(case=label, exit=run.returncode, decision=bundle['envelope']['decision'], required_scopes=bundle['envelope']['coverage']['requiredScopes']))
    request['candidate_oid'] = '0'*40
    unknown = subprocess.run([str(args.gitguard_bin.resolve()), 'check'], input=json.dumps(request).encode(), env=env, capture_output=True, timeout=30)
    (args.output/'gg-unknown-object.stdout').write_bytes(unknown.stdout)
    (args.output/'gg-unknown-object.stderr').write_bytes(unknown.stderr)
    comparisons.append(dict(case='gg-unknown-object', exit=unknown.returncode, candidate_oid='0'*40, transport='actual CLI prebinding error'))
    assert before == state(), 'observation modified fixture files'
    git('bundle', 'create', str(args.output.resolve()/'source.bundle'), '--all')
    hashes = {str(p.relative_to(provider)): hashlib.sha256(p.read_bytes()).hexdigest() for p in provider.rglob('*.py') if '.git' not in p.parts}
    result = dict(provider_commit=PIN, plugin_version=json.loads((provider/'plugin.json').read_text())['version'], provider_python_hashes=hashes,
          gitguard_binary_sha256=hashlib.sha256(args.gitguard_bin.read_bytes()).hexdigest(), candidate_oid=head, base_oid=base, native=records, gitguard=comparisons,
          source_bytes_unchanged=True, internal_error_profile='fault-only invalid UTF8 message file, unchanged provider',
          policy_sha256=hashlib.sha256((root/'.gitflow/workflow.json').read_bytes()).hexdigest(), independently_prepared_native_policy_hash=expected_native_hash, python_version=sys.version, git_version=git('--version'))
    assert records[0]['stdout']['policy_sha256'] == expected_native_hash
    assert not subprocess.check_output(['git','-C',str(provider),'status','--porcelain'])
    (args.output/'capture.json').write_text(json.dumps(result, indent=2, ensure_ascii=False)+'\n')
    (args.output/'workflow.json').write_bytes((root/'.gitflow/workflow.json').read_bytes())
    print(json.dumps({r['case']:r['exit'] for r in records}))
