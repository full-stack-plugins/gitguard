"""Real paired entrypoints: install owns guidance; init only selects XML policy."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

BIN=Path(os.environ.get('GITGUARD_BIN','target/debug/gitguard'+('.exe' if os.name=='nt' else ''))).resolve()
PLUGIN=Path(os.environ.get('GITGUARD_ADAPTER_ROOT',str(Path(__file__).resolve().parents[2]/'gitguard-plugin'))).resolve()


class GuidanceCoexistence(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.project=self.root/'project';self.project.mkdir()
        self.home=self.root/'home';self.home.mkdir()
        self.env={**os.environ,'GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':os.devnull}
        subprocess.run(['git','init','-b','main',str(self.project)],env=self.env,check=True,capture_output=True)
        self.agents=self.project/'AGENTS.md';self.agents.write_bytes(b'Human guidance\r\n')

    def snapshot(self):
        return {str(p.relative_to(self.project)):p.read_bytes() for p in self.project.rglob('*') if p.is_file()}

    def init(self,*args,code=0):
        p=subprocess.run([sys.executable,str(PLUGIN/'scripts/gitflow.py'),'init',str(self.project),'--profile','github-flow','--apply','--json',*args],env=self.env,capture_output=True,text=True,encoding='utf-8',timeout=30)
        self.assertEqual(p.returncode,code,p.stdout+p.stderr);return json.loads(p.stdout)

    def engine(self,*args,code=0):
        p=subprocess.run([str(BIN),*args,'--project',str(self.project),'--home',str(self.home),'--gitflow-root',str(PLUGIN),'--python',sys.executable],env=self.env,capture_output=True,text=True,encoding='utf-8',timeout=30)
        self.assertEqual(p.returncode,code,p.stdout+p.stderr);return json.loads(p.stdout)

    def assert_reference_guidance(self):
        raw=self.agents.read_bytes()
        self.assertTrue(raw.startswith(b'Human guidance\r\n'))
        self.assertNotIn(b'GITGUARD_POLICY_GUIDANCE',raw)
        self.assertEqual(raw.count(b'<!-- GITGUARD_START -->'),1)
        self.assertIn(b'<!-- owner: gitguard-runtime-install/v1 -->',raw)
        for copied in ('允许合入','创建来源','commit_message_pattern','merge_into'):
            self.assertNotIn(copied,raw.decode())
        self.assertEqual(self.engine('doctor','--integration')['status'],'configured')

    def test_init_then_install_repeated_twice_preserves_install_owned_guidance(self):
        for _ in range(2):
            before=self.agents.read_bytes();self.init();self.assertEqual(self.agents.read_bytes(),before)
            self.engine('install','--agents','codex')
        self.assert_reference_guidance();before=self.snapshot()
        self.init();self.engine('install','--agents','codex')
        self.assertEqual(self.snapshot(),before)

    def test_install_then_init_repeated_twice_preserves_install_owned_guidance(self):
        for _ in range(2):
            self.engine('install','--agents','codex');before=self.agents.read_bytes()
            self.init();self.assertEqual(self.agents.read_bytes(),before)
        self.assert_reference_guidance();before=self.snapshot()
        self.engine('install','--agents','codex');self.init()
        self.assertEqual(self.snapshot(),before)

    def test_unknown_block_init_ignores_install_refuses_without_writes(self):
        self.agents.write_text('Human\n<!-- GITGUARD_START -->\nunknown\n<!-- GITGUARD_END -->\n')
        content=self.agents.read_bytes();self.init();self.assertEqual(self.agents.read_bytes(),content)
        before=self.snapshot()
        self.engine('install','--agents','codex',code=4);self.assertEqual(self.snapshot(),before)

    def test_edited_runtime_init_ignores_install_refuses_without_writes(self):
        self.engine('install','--agents','codex')
        self.agents.write_bytes(self.agents.read_bytes().replace(b'## gitguard',b'## edited gitguard'))
        content=self.agents.read_bytes();self.init();self.assertEqual(self.agents.read_bytes(),content)
        before=self.snapshot()
        self.engine('install','--agents','codex',code=4);self.assertEqual(self.snapshot(),before)

    def test_init_never_adopts_or_migrates_legacy_guidance(self):
        old=b'Human guidance\r\n<!-- GITGUARD_START -->\n## GitGuard\nlegacy init guidance\n<!-- GITGUARD_END -->\n'
        self.agents.write_bytes(old);self.init();self.assertEqual(self.agents.read_bytes(),old)
        before=self.snapshot();self.engine('install','--agents','codex',code=4)
        self.assertEqual(self.snapshot(),before)

    def test_absent_agents_stays_absent_until_install(self):
        self.agents.unlink();self.init();self.assertFalse(self.agents.exists())
        xml=(self.project/'.git/gitflow.xml').read_bytes()
        self.engine('install','--agents','codex')
        self.assertEqual(self.agents.read_text(encoding='utf-8').count('<!-- GITGUARD_START -->'),1)
        self.assertEqual((self.project/'.git/gitflow.xml').read_bytes(),xml)

    def test_broken_runtime_marker_is_preserved_by_both_entrypoints(self):
        self.engine('install','--agents','codex')
        self.agents.write_bytes(self.agents.read_bytes().replace(b'<!-- GITGUARD_END -->',b''))
        content=self.agents.read_bytes();self.init();self.assertEqual(self.agents.read_bytes(),content)
        before=self.snapshot();self.engine('install','--agents','codex',code=4)
        self.assertEqual(self.snapshot(),before)

    def test_legacy_receipt_owned_runtime_block_migrates_by_explicit_sync(self):
        self.engine('install','--agents','codex')
        receipt_file=self.project/'.guard-install/gitguard.json'
        receipt=json.loads(receipt_file.read_text(encoding='utf-8'))
        entry=next(e for e in receipt['entries'] if e['kind']=='markdown' and Path(e['path']).samefile(self.agents))
        current=entry['content']
        # A recorded v0.1.0 block fixture: actual ownership receipt, no mock executor.
        old='<!-- GITGUARD_START -->\n## gitguard\n\n- 先读取已激活工作流。\n<!-- GITGUARD_END -->'
        self.agents.write_bytes(self.agents.read_bytes().replace(current.encode(),old.encode()))
        entry['content']=old;receipt_file.write_text(json.dumps(receipt))
        before=self.agents.read_bytes();self.init();self.assertEqual(self.agents.read_bytes(),before)
        self.engine('instructions','sync');self.init();self.engine('instructions','sync')
        self.assert_reference_guidance()


if __name__=='__main__':unittest.main()
