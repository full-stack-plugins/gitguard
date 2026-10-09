#!/usr/bin/env python3
"""Local storage observation schema; no platform receipt or authority."""
import copy
import json
from pathlib import Path
from jsonschema import Draft202012Validator
root = Path(__file__).resolve().parents[1]
schema = json.loads((root / 'schemas/operation-receipt.schema.json').read_text())
Draft202012Validator.check_schema(schema)
v = Draft202012Validator(schema)
good = json.loads((root / 'fixtures/schema/operation-intent-valid.json').read_text())
count = 0
for generation, state in enumerate(['prepared', 'attempt_recorded', 'recovery_required']):
    item = dict(good, generation=generation, state=state)
    assert not list(v.iter_errors(item))
    count += 1
    for other in range(3):
        if other != generation:
            assert list(v.iter_errors(dict(item, generation=other)))
            count += 1
for field in good:
    item = copy.deepcopy(good)
    del item[field]
    assert list(v.iter_errors(item))
    count += 1
for field, value in [('approved', True), ('state', 'succeeded'), ('state', 'unknown'),
                     ('generation', 3), ('operation_id', ''), ('operation_id', 'x'*129),
                     ('operation_id', '\n'), ('request_digest', 'b'*64), ('store_id', 'sha256:'+'a'*64),
                     ('api_version', 'future')]:
    assert list(v.iter_errors(dict(good, **{field: value})))
    count += 1
print(json.dumps({'cases_passed': count, 'platform_receipt': False, 'authorization': False}))
