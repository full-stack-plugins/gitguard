"""Run the pinned shared installation suite with its one JSON fixture migrated.

All upstream test methods remain present. Only the old GitFlow policy fixture is
replaced; no production adapter or shared GuardEngine source is modified.
"""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tomllib
import unittest

SOURCE = Path(__file__).resolve().parents[2] / 'guardengine/scripts/verify_guard_installation.py'
spec = importlib.util.spec_from_file_location('guardengine_installation_acceptance', SOURCE)
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)  # Upstream argparse consumes the forwarded CLI arguments.
EXPECTED_METHODS = {
    'test_all_hosts_coexist_repeat_and_uninstall',
    'test_preview_and_user_scope_and_binary_lifecycle',
    'test_conflicts_do_not_partially_write',
    'test_mcp_protocol_and_cli_context_match',
    'test_untrusted_receipt_does_not_execute_adapter',
    'test_unknown_options_locks_and_binary_ownership',
    'test_domain_capability_boundaries_are_truthful',
    'test_six_installed_blocks_pass_specguard_and_sync_preserves_human_text',
    'test_linked_host_configuration_is_rejected',
    'test_gitflow_profiles_drift_and_refresh',
}


class XMLInstallation(shared.Installation):
    def test_install_owns_guidance_and_init_only_xml(self):
        if 'gitguard' not in shared.TOOLS or not shared.config.gitflow_root:
            self.skipTest('explicit existing GitFlow source required')
        plugin = shared.config.gitflow_root.resolve()
        python = Path(sys.executable).resolve()
        subprocess.run(['git', 'init', '--quiet', str(self.project)], check=True, capture_output=True)
        filters = subprocess.run(['git', '-C', str(self.project), 'config', '--name-only',
                                  '--get-regexp', r'^filter\..*\.(clean|process)$'],
                                 capture_output=True, text=True, check=False)
        for key in filters.stdout.splitlines():
            subprocess.run(['git', '-C', str(self.project), 'config', '--local', key, ''], check=True, capture_output=True)
        (self.project/'AGENTS.md').write_bytes(b'Human guidance\r\n')
        initialized = subprocess.run([str(python), str(plugin / 'scripts/gitflow.py'), 'init',
                                      str(self.project), '--profile', 'github-flow', '--apply', '--json'],
                                     capture_output=True, timeout=45)
        self.assertEqual(initialized.returncode, 0, initialized.stdout + initialized.stderr)
        self.assertEqual((self.project/'AGENTS.md').read_bytes(),b'Human guidance\r\n')
        guidance=(self.project/'AGENTS.md').read_bytes()
        self.call('gitguard','install','--agents','codex,claude,gemini',
                  '--gitflow-root',plugin,'--python',python)
        self.assertIn(guidance.rstrip(b'\n'),(self.project/'AGENTS.md').read_bytes())
        stable=self.snapshot()
        repeated=subprocess.run([str(python),str(plugin/'scripts/gitflow.py'),'init',str(self.project),
                                 '--profile','github-flow','--apply','--json'],capture_output=True,timeout=45)
        self.assertEqual(repeated.returncode,0,repeated.stdout+repeated.stderr)
        self.call('gitguard','install','--agents','codex,claude,gemini',
                  '--gitflow-root',plugin,'--python',python)
        self.assertEqual(self.snapshot(),stable)

    def test_gitflow_profiles_drift_and_refresh(self):
        if 'gitguard' not in shared.TOOLS or not shared.config.gitflow_root:
            self.skipTest('explicit existing GitFlow source required')
        plugin = shared.config.gitflow_root.resolve()
        python = Path(sys.executable).resolve()
        adapter = ('--gitflow-root', plugin, '--python', python)
        subprocess.run(['git', 'init', '--quiet', str(self.project)], check=True, capture_output=True)
        # Preserve the upstream isolation of inherited executable clean filters.
        filters = subprocess.run(['git', '-C', str(self.project), 'config', '--name-only',
                                  '--get-regexp', r'^filter\..*\.(clean|process)$'],
                                 capture_output=True, text=True, check=False)
        for key in filters.stdout.splitlines():
            subprocess.run(['git', '-C', str(self.project), 'config', '--local', key, ''],
                           check=True, capture_output=True)
        sys.path.insert(0, str(plugin / 'scripts'))
        from gitflow.xml_policy import encode, decode
        xml = self.project / '.git/gitflow.xml'
        github = json.loads((plugin / 'profiles/github-flow.json').read_text())
        classic = json.loads((plugin / 'profiles/classic-gitflow.json').read_text())
        for policy in (github, classic):
            # Real plugin init belongs to a separate staging repository. Only
            # policy XML is provisioned into the independent engine fixture.
            staging = self.root / ('policy-staging-' + policy['profile'])
            staging.mkdir()
            subprocess.run(['git', 'init', '--quiet', str(staging)], check=True, capture_output=True)
            for key in filters.stdout.splitlines():
                subprocess.run(['git', '-C', str(staging), 'config', '--local', key, ''], check=True, capture_output=True)
            initialized = subprocess.run([str(python), str(plugin / 'scripts/gitflow.py'), 'init',
                                          str(staging), '--profile', policy['profile'], '--apply', '--json'],
                                         capture_output=True, timeout=45)
            self.assertEqual(initialized.returncode, 0, initialized.stdout + initialized.stderr)
            self.assertFalse((staging/'AGENTS.md').exists())
            generated = (staging / '.git/gitflow.xml').read_bytes()
            self.assertEqual(decode(generated), policy)
            xml.write_bytes(generated)
            context = self.call('gitguard', 'context', *adapter)
            self.assertEqual(context['domain']['status'], 'active', context)
            self.assertNotIn('roles',context['domain'])
            self.assertEqual(context['domain']['policy_sha256'], __import__('hashlib').sha256(json.dumps(policy,sort_keys=True,ensure_ascii=False,separators=(',',':')).encode()).hexdigest())
            self.call('gitguard', 'install', '--agents', 'codex,claude,gemini', *adapter)
            self.assertFalse((staging/'AGENTS.md').exists())
            text = (self.project / 'AGENTS.md').read_text(encoding='utf-8')
            self.assertIn(policy['profile'], text)
            self.assertEqual(self.call('gitguard', 'doctor', '--integration', *adapter)['status'], 'configured')
        self.assertNotIn('允许合入', text)
        self.assertNotIn('创建来源', text)
        self.assertNotIn('github-flow', text)
        classic['revision'] += 1
        xml.write_bytes(encode(classic))
        self.assertEqual(self.call('gitguard', 'context', *adapter)['domain']['revision'], classic['revision'])
        self.call('gitguard', 'doctor', '--integration', *adapter, expected=3)
        self.call('gitguard', 'instructions', 'sync', *adapter)
        self.assertEqual(self.call('gitguard', 'doctor', '--integration', *adapter)['status'], 'configured')
        # Legacy candidates/snapshots cannot override XML or serve as fallback.
        state = self.project / '.gitflow/state'
        state.mkdir(parents=True, exist_ok=True)
        import hashlib
        digest = hashlib.sha256(json.dumps(github, sort_keys=True, ensure_ascii=False, separators=(',', ':')).encode()).hexdigest()
        (state / 'activation.json').write_text(json.dumps({'schema_version': '1.0.0', 'mode': 'shared', 'policy': github, 'sha256': digest}))
        (self.project / '.gitflow/workflow.json').write_text(json.dumps(github))
        context = self.call('gitguard', 'context', *adapter)
        self.assertEqual(context['domain']['status'], 'active')
        self.assertEqual(context['domain']['profile'], classic['profile'])
        self.assertEqual(context['domain']['revision'], classic['revision'])
        self.assertEqual(self.call('gitguard', 'doctor', '--integration', *adapter)['status'], 'configured')
        good = xml.read_bytes()
        for raw in (b'<policy>', None):
            if raw is None: xml.unlink()
            else: xml.write_bytes(raw)
            context = self.call('gitguard', 'context', *adapter)
            self.assertEqual(context['domain']['status'], 'unverified', context)
            self.call('gitguard', 'doctor', '--integration', *adapter, expected=3)
        xml.write_bytes(good)
        self.assertEqual(self.call('gitguard', 'doctor', '--integration', *adapter)['status'], 'configured')
        # Execute the actual generated, pinned MCP command, retaining upstream assertions.
        entry = tomllib.loads((self.project / '.codex/config.toml').read_text(encoding='utf-8'))['mcp_servers']['gitguard']
        self.assertEqual(Path(entry['command']).resolve(), self.binary('gitguard').resolve())
        self.assertIn(str(plugin), entry['args'])
        self.assertIn(str(python), entry['args'])
        messages = [
            {'jsonrpc':'2.0','id':1,'method':'initialize','params':{'protocolVersion':'2024-11-05','capabilities':{},'clientInfo':{'name':'test','version':'1'}}},
            {'jsonrpc':'2.0','method':'notifications/initialized'},
            {'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'gitguard_context','arguments':{'project':str(self.project)}}},
        ]
        result = subprocess.run([entry['command'], *entry['args']], input=('\n'.join(map(json.dumps,messages))+'\n').encode(), capture_output=True, timeout=45)
        self.assertEqual(result.returncode, 0, result.stderr)
        reply = json.loads(result.stdout.splitlines()[-1])
        self.assertEqual(json.loads(reply['result']['content'][0]['text'])['domain']['status'], 'active')


if __name__ == '__main__':
    loader = unittest.TestLoader()
    upstream = set(loader.getTestCaseNames(shared.Installation))
    actual = set(loader.getTestCaseNames(XMLInstallation))
    extra = {'test_install_owns_guidance_and_init_only_xml'}
    if upstream != EXPECTED_METHODS or actual != upstream | extra:
        raise SystemExit(f'Shared suite method set changed: upstream={sorted(upstream)}, wrapper={sorted(actual)}')
    print(f'Preserved all {len(upstream)} shared test methods; one XML fixture override and one install/init responsibility regression.', flush=True)
    result = unittest.TextTestRunner(verbosity=2).run(loader.loadTestsFromTestCase(XMLInstallation))
    raise SystemExit(not result.wasSuccessful())
