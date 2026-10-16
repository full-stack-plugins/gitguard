"""真实 CLI/MCP：激活策略负例、修正请求、复验；不改变被测仓库。"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

BIN = Path(os.environ.get('GITGUARD_BIN', 'target/debug/gitguard' + ('.exe' if os.name == 'nt' else ''))).resolve()
ADAPTER = Path(__file__).resolve().parents[2] / 'gitflow-plugin'


class PolicyCheck(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name)
        self.git('init', '-b', 'feature/task')
        # 仅隔离临时夹具中继承的宿主 LFS 等命令，不修改用户 Git 配置。
        inherited = subprocess.run(['git', '-C', str(self.repo), 'config', '--get-regexp',
                                    r'^filter\..*\.(clean|process)$'], capture_output=True, text=True)
        for key in {line.split(maxsplit=1)[0] for line in inherited.stdout.splitlines() if line}:
            self.git('config', key, '')
        self.git('-c', 'user.name=Test', '-c', 'user.email=test@example.org', 'commit', '--allow-empty', '-m', 'seed')
        self.git('branch', 'main')
        self.git('branch', 'develop')
        self.policy = json.loads((ADAPTER / 'profiles/classic-gitflow.json').read_text())
        self.policy['commit_message_pattern'] = r'feat: .+'
        self.activate()

    def git(self, *args):
        return subprocess.check_output(['git', '-C', str(self.repo), *args], stderr=subprocess.PIPE)

    def activate(self):
        import hashlib
        digest = hashlib.sha256(json.dumps(self.policy, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
        state = self.repo / '.gitflow/state'
        state.mkdir(parents=True, exist_ok=True)
        (state / 'activation.json').write_text(json.dumps({'schema_version': '1.0.0', 'mode': 'local', 'policy': self.policy, 'sha256': digest}))

    def request(self, **kwargs):
        request = dict(project=str(self.repo), gitflow_root=str(ADAPTER), python=sys.executable)
        request.update(kwargs)
        return request

    def cli(self, request, env=None):
        args = [str(BIN), 'policy', 'check']
        for key, value in request.items():
            args.extend(['--' + key.replace('_', '-'), value])
        result = subprocess.run(args, capture_output=True, text=True, encoding='utf-8', env=env)
        self.assertIn(result.returncode, (0, 2, 3), result.stderr)
        return json.loads(result.stdout)

    def mcp(self, request, env=None):
        messages = [dict(jsonrpc='2.0', id=1, method='initialize', params=dict(protocolVersion='2024-11-05', capabilities={}, clientInfo=dict(name='test', version='1'))),
                    dict(jsonrpc='2.0', method='notifications/initialized'),
                    dict(jsonrpc='2.0', id=2, method='tools/call', params=dict(name='gitguard_policy_check', arguments=request))]
        run = subprocess.run([str(BIN), 'serve', '--mcp', '--project', str(self.repo)],
                             input=''.join(json.dumps(m) + '\n' for m in messages), capture_output=True, text=True, encoding='utf-8', env=env)
        reply = [json.loads(line) for line in run.stdout.splitlines() if json.loads(line).get('id') == 2][0]
        return json.loads(reply['result']['content'][0]['text'])

    def test_negative_fix_recheck_cli_mcp(self):
        before = self.git('status', '--porcelain')
        for bad, good in [
            (dict(action='branch', branch='invalid'), dict(action='branch', branch='feature/task')),
            (dict(action='merge', source='feature/task', target='main'), dict(action='merge', source='feature/task', target='develop')),
            (dict(action='commit', message='bad'), dict(action='commit', message='feat: implemented')),
        ]:
            for fields, decision in [(bad, 'deny'), (good, 'allow')]:
                request = self.request(**fields)
                cli = self.cli(request)
                self.assertEqual(cli['decision'], decision, cli)
                self.assertEqual(cli, self.mcp(request))
                self.assertEqual(cli['quality_decision'], 'not_evaluated')
        self.assertEqual(before, self.git('status', '--porcelain'))

    def test_missing_activation_and_drift_never_allow(self):
        (self.repo / '.gitflow/state/activation.json').unlink()
        self.assertEqual(self.cli(self.request(action='branch', branch='feature/task'))['decision'], 'unverified')
        self.activate()
        active = self.repo / '.gitflow/state/activation.json'
        data = json.loads(active.read_text())
        data['mode'] = 'shared'
        active.write_text(json.dumps(data))
        candidate = dict(self.policy, revision=2)
        (self.repo / '.gitflow/workflow.json').write_text(json.dumps(candidate))
        result = self.cli(self.request(action='branch', branch='feature/task'))
        self.assertEqual(result['decision'], 'unverified')
        self.assertEqual(result['code'], 'policy_drift')

    def test_protected_branch_unknown_source_and_input_bounds(self):
        self.git('checkout', 'main')
        result = self.cli(self.request(action='commit', message='feat: blocked'))
        self.assertEqual(result['decision'], 'deny')
        self.assertIn('protected_branch_commit', [reason['code'] for reason in result['reasons']])
        result = self.cli(self.request(action='merge', source='feature/missing', target='develop'))
        self.assertEqual(result['decision'], 'unverified')
        self.assertEqual(result['code'], 'branch_identity_unknown')
        result = self.cli(self.request(action='branch', branch='feature/../invalid'))
        self.assertEqual(result['decision'], 'deny')
        result = self.cli(self.request(action='branch', branch='feature/task', gitflow_root=str(self.repo)))
        self.assertEqual(result['decision'], 'unverified')
        result = subprocess.run([str(BIN), 'policy', 'check', '--action', 'commit', '--apply', 'true'], capture_output=True)
        self.assertEqual(result.returncode, 4)


    def test_project_clean_filter_never_executes_during_read_only_check(self):
        (self.repo / '.gitattributes').write_text('data.txt filter=guard-review\n')
        (self.repo / 'data.txt').write_text('initial\n')
        self.git('add', '.gitattributes', 'data.txt')
        self.git('-c', 'user.name=Test', '-c', 'user.email=test@example.org', 'commit', '-m', 'fixture')
        marker = self.repo / 'filter-executed'
        script = self.repo / 'filter.py'
        script.write_text('import pathlib,sys\npathlib.Path(' + repr(str(marker)) + ').write_text("executed")\nsys.stdout.buffer.write(sys.stdin.buffer.read())\n')
        command = '"' + sys.executable.replace('\\', '/') + '" "' + str(script).replace('\\', '/') + '"'
        (self.repo / 'data.txt').write_text('changed\n')
        for kind in ('clean', 'process'):
            with self.subTest(filter=kind):
                self.git('config', 'filter.guard-review.' + kind, command)
                result = self.cli(self.request(action='branch', branch='feature/task'))
                self.assertFalse(marker.exists(), 'read-only check executed the project filter')
                self.assertEqual(result['decision'], 'unverified', result)
                self.assertEqual(result['code'], 'executable_filter_unverified')
                self.assertEqual(self.mcp(self.request(action='branch', branch='feature/task')), result)
                self.assertFalse(marker.exists(), 'MCP executed the project filter')
                self.git('config', 'filter.guard-review.' + kind, '')

    def test_commit_author_environment_is_the_checked_identity(self):
        self.git('config', 'user.name', 'Allowed')
        self.git('config', 'user.email', 'allowed@example.org')
        self.policy.update(schema_version='2.0.0', extends=None,
                           rules={'GF102': {'severity': 'error', 'options': {'pattern': r'allowed@example[.]org'}}})
        self.activate()
        environment = dict(os.environ, GIT_AUTHOR_EMAIL='forbidden@example.org')
        result = self.cli(self.request(action='commit', message='feat: verified'), env=environment)
        self.assertEqual(result['decision'], 'deny', result)
        check = next(item for item in result['checks'] if item['rule_id'] == 'GF102')
        self.assertEqual(check['actual'], 'forbidden@example.org')
        self.assertEqual(check['status'], 'fail')
        self.assertEqual(result, self.mcp(self.request(action='commit', message='feat: verified'), env=environment))


if __name__ == '__main__':
    unittest.main()
