#!/usr/bin/env python3
"""Local wire schema checks; no authentication or execution."""
import copy
import json
from pathlib import Path
from jsonschema import Draft202012Validator
root = Path(__file__).resolve().parents[1]
schema = json.loads((root / 'schemas/operation-grant.schema.json').read_text())
Draft202012Validator.check_schema(schema)
validator = Draft202012Validator(schema)
good = json.loads((root / 'fixtures/schema/operation-grant-valid.json').read_text())
passed = 0
for action in ['create_worktree', 'create_branch', 'push', 'merge']:
    value = copy.deepcopy(good)
    value['action'] = action
    if action in ['create_worktree', 'create_branch']:
        value['expected_target_oid'] = None
    if action == 'create_worktree':
        value['target'] = 'worktrees/task'
    assert not list(validator.iter_errors(value)), action
    passed += 1
for field in good:
    value = copy.deepcopy(good)
    del value[field]
    assert list(validator.iter_errors(value)), ('missing', field)
    passed += 1
for field, value in [
    ('approved', True), ('api_version', 'future'), ('issuer', ''),
    ('actor', 'x' * 129), ('action', 'force_push'), ('repo_id', '\n'),
    ('candidate_oid', 'A' * 40), ('binding_digest', 'sha256:' + 'a' * 64),
    ('target', '../escape'), ('target', 'refs/heads/a//b'),
    ('target', 'refs/heads/a.lock'), ('target', 'refs/heads/a.'),
    ('target', '/absolute'), ('target', 'refs/heads/.hidden'),
    ('expected_target_oid', None), ('expected_target_oid', 'a' * 39),
    ('operation_id', ''), ('not_before', -1), ('expires_at', 0),
    ('revoked', 'false'),
]:
    changed = copy.deepcopy(good)
    changed[field] = value
    assert list(validator.iter_errors(changed)), ('invalid', field, value)
    passed += 1
print(json.dumps({'schema': 'operation-grant.schema.json', 'cases_passed': passed, 'authentication': False}))
