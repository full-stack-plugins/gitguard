"""Fixture-runner integrity tests; no producer or production authority is synthesized."""
import hashlib
import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("joint_fixture", Path(__file__).with_name("run.py"))
joint = importlib.util.module_from_spec(spec)
spec.loader.exec_module(joint)


class Integrity(unittest.TestCase):
    def test_changed_original_artifact_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "actual"
            shutil.copytree(Path(__file__).with_name("actual"), root)
            joint.verify_corpus(root)
            (root / "specguard/report.json").write_bytes(b"{}")
            with self.assertRaisesRegex(ValueError, "artifact mismatch"):
                joint.verify_corpus(root)

    def test_wrong_tool_pin_is_rejected_before_execution(self):
        with self.assertRaisesRegex(ValueError, "controller pin"):
            joint.Tool("/usr/bin/git", "0" * 64)

    def test_tool_path_replacement_does_not_replace_opened_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "git"
            shutil.copyfile("/usr/bin/git", path)
            path.chmod(0o700)
            tool = joint.Tool(path, hashlib.sha256(path.read_bytes()).hexdigest())
            try:
                path.unlink()
                path.write_text("#!/bin/sh\necho replaced\n")
                path.chmod(0o700)
                code, stdout, _ = tool.run(["--version"])
                self.assertEqual(code, 0)
                self.assertTrue(stdout.startswith(b"git version "))
            finally:
                tool.close()


if __name__ == "__main__":
    unittest.main()
