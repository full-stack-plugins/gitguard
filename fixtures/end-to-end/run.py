#!/usr/bin/env python3
"""Opt-in current consumers over unchanged, provenance-pinned actual producer captures.

This is a local fixture replay, not a producer runner or an authority service.
"""
import argparse
import concurrent.futures
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import tempfile

PROVENANCE_SHA256 = "0be2683c5afa2be89c2b528a3316fcf733b3f91bc0135fd7bd4394808260c743"
PROVIDERS = ("archguard", "codeguard", "gitguard", "specguard", "testguard")
NOW = 2_000_000_000
TIME = "2033-05-18T03:33:20Z"
LIMIT = 16 * 1024 * 1024


def sha(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode()


def write(path, value):
    path.write_bytes(encoded(value))


def read(path):
    data = path.read_bytes()
    if len(data) > LIMIT:
        raise ValueError("fixture artifact budget")
    return json.loads(data)


def environment():
    return {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_OPTIONAL_LOCKS": "0",
            "GIT_TERMINAL_PROMPT": "0", "GIT_NO_LAZY_FETCH": "1"}


class Tool:
    def __init__(self, path, expected):
        self.fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        try:
            metadata = os.fstat(self.fd)
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > 64 * 1024 * 1024:
                raise ValueError("unsupported tool file")
            digest = hashlib.sha256()
            while data := os.read(self.fd, 1024 * 1024):
                digest.update(data)
            self.digest = digest.hexdigest()
            if self.digest != expected:
                raise ValueError("tool bytes differ from controller pin")
        except BaseException:
            self.close()
            raise

    def close(self):
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None

    def run(self, args, stdin=None):
        # Open descriptor, not a re-resolved binary path. Controller protects in-place edits.
        with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
            process = subprocess.Popen([f"/proc/self/fd/{self.fd}", *args],
                                       stdin=subprocess.PIPE, stdout=out, stderr=err,
                                       pass_fds=(self.fd,), env=environment(), start_new_session=True)
            try:
                process.communicate(stdin, timeout=45)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate()
                raise ValueError("consumer deadline exceeded") from None
            if out.tell() > LIMIT or err.tell() > LIMIT:
                raise ValueError("consumer output budget")
            out.seek(0)
            err.seek(0)
            return process.returncode, out.read(), err.read()


def verify_corpus(root):
    raw = (root / "PROVENANCE.json").read_bytes()
    if sha(raw) != PROVENANCE_SHA256:
        raise ValueError("retained producer provenance differs")
    metadata = json.loads(raw)
    for group in ("protectedBeforeResults", "artifacts"):
        for name, expected in metadata[group].items():
            path = root / name
            if not path.resolve().is_relative_to(root.resolve()) or path.is_symlink():
                raise ValueError("artifact path escaped retained corpus")
            if sha(path.read_bytes()) != expected:
                raise ValueError(f"retained artifact mismatch: {name}")
    return metadata


def tree_digest(root):
    values = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise ValueError("unexpected fixture symlink")
        if path.is_file():
            values.append((str(path.relative_to(root)), sha(path.read_bytes())))
    return sha(encoded(values))


def run(options):
    corpus = Path(__file__).resolve().parent / "actual"
    provenance = verify_corpus(corpus)
    output = Path(options.output)
    output.mkdir(mode=0o700, parents=False, exist_ok=False)
    tools = {}
    try:
        tools["gitguard"] = Tool(options.gitguard, options.gitguard_sha256)
        tools["flowguard"] = Tool(options.flowguard, options.flowguard_sha256)
        with tempfile.TemporaryDirectory(prefix="gitguard-joint-") as temporary:
            root = Path(temporary)
            repository = root / "repository"
            cloned = subprocess.run(["/usr/bin/git", "clone", "--quiet", str(corpus / "source.bundle"), str(repository)],
                                    env=environment(), stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
            if cloned.returncode != 0:
                raise ValueError("fixture bundle restoration failed")
            before = tree_digest(repository)
            inputs = output / "inputs"
            shutil.copytree(corpus, inputs)
            current = output / "current-gitguard"
            current.mkdir()
            request = read(inputs / "gitguard/request.json")
            request["repo_root"] = str(repository)
            write(current / "request.json", request)
            code, raw, err = tools["gitguard"].run(["check"], encoded(request))
            (current / "stdout.json").write_bytes(raw)
            (current / "stderr.txt").write_bytes(err)
            if code != 0:
                raise ValueError(f"current candidate check failed: {code}")
            fresh = json.loads(raw)
            if fresh["domain"]["candidate"] != read(inputs / "gitguard/candidate.json"):
                raise ValueError("current candidate differs from retained producer scope")
            if fresh["envelope"]["binding"]["candidateOid"] != provenance["candidate"]:
                raise ValueError("retained candidate identity mismatch")
            binding = read(inputs / "binding.json")
            frozen = read(inputs / "frozen.json")
            policies = read(inputs / "policies.json")
            scopes = read(inputs / "scopes.json")
            envelopes = {provider: read(inputs / provider / "envelope.json") for provider in PROVIDERS}
            base_request = {
                "version": "flowguard.cli-request/v1alpha1",
                "invocation": {"repo_candidates": [binding["repoId"]], "task_candidates": [binding["taskId"]],
                               "worktree_id": binding["worktreeId"], "requirement_ids": binding["requirementIds"],
                               "candidate_oid": binding["candidateOid"], "base_oid": binding["baseOid"]},
                "candidate": "gitguard/candidate.json", "frozen": "frozen.json", "frozen_digest": frozen["digest"],
                "sources": "sources.json", "sources_digest": "sha256:" + sha((inputs / "sources.json").read_bytes()),
                "action": "commit", "started_at": TIME, "finished_at": TIME, "now": NOW,
                "specialists": [{"scope": scopes[p], "producer": envelopes[p]["producer"],
                                 "producer_principals": policies[scopes[p]]["producer_principals"],
                                 "approval_principals": policies[scopes[p]]["approval_principals"],
                                 "evidence": {key: f"{p}/{key}.json" for key in ("envelope", "contract", "facts", "report")}}
                                for p in PROVIDERS]}
            authority = {"version": "flowguard.authority-fixture/v1alpha1", "producers": [
                {"principal": "fixture-ci", "producer": envelopes[p]["producer"],
                 "envelope_digest": "sha256:" + sha(encoded(envelopes[p])), "issued_at": NOW - 10,
                 "expires_at": NOW + 100, "revoked": False} for p in PROVIDERS], "approvals": [
                {"reference": "fixture:actual-spec-review", "principal": "fixture-reviewer", "purpose": "review", "action": "commit",
                 "binding": envelopes["specguard"]["binding"],
                 "contract_digest": envelopes["specguard"]["artifacts"]["contract"]["digest"],
                 "issued_at": NOW - 10, "expires_at": NOW + 100, "revoked": False}]}
            cases = {}

            def execute(name):
                request = copy.deepcopy(base_request)
                records = copy.deepcopy(authority)
                if name == "missing_approval":
                    records["approvals"] = []
                elif name == "expired_approval":
                    records["approvals"][0]["expires_at"] = NOW
                elif name == "revoked_approval":
                    records["approvals"][0]["revoked"] = True
                elif name == "missing_specialist":
                    request["specialists"][0]["evidence"] = None
                elif name == "other_task":
                    request["invocation"]["task_candidates"] = ["another-task"]
                elif name == "other_candidate":
                    request["invocation"]["candidate_oid"] = binding["baseOid"]
                case = output / name
                case.mkdir()
                write(case / "request.json", request)
                write(case / "authority.json", records)
                # Request is rooted with the immutable corpus; only transport request bytes are new.
                input_name = f"request-{name}.json"
                write(inputs / input_name, request)
                args = ["gate", "check", "--repo", str(repository), "--input-root", str(inputs),
                        "--request", input_name, "--request-digest", "sha256:" + sha(encoded(request)), "--run-id", f"joint-{name}",
                        "--local-fixture-authority", "--fixture-authority-root", str(case),
                        "--fixture-authority", "authority.json", "--fixture-authority-digest", "sha256:" + sha(encoded(records))]
                if name == "cancelled":
                    args.append("--cancel")
                code, raw, err = tools["flowguard"].run(args)
                (case / "stdout.json").write_bytes(raw)
                (case / "stderr.txt").write_bytes(err)
                value = json.loads(raw) if raw else None
                expected = 0 if name == "allow" else 2 if name == "missing_specialist" else 4
                if code != expected:
                    raise ValueError(f"case {name}: expected {expected}, got {code}; inspect capture")
                if value and (value["execution_authorized"] is not False or value["authority_profile"] != "local_fixture"):
                    raise ValueError("gate must remain fixture-only and unauthorizing")
                if name == "cancelled" and value["envelope"]["decision"] is not None:
                    raise ValueError("cancelled gate manufactured a decision")
                return name, {"exit": code, "stdout_sha256": sha(raw), "stderr_sha256": sha(err),
                              "request_sha256": sha(encoded(request)), "authority_sha256": sha(encoded(records)),
                              "decision": value["envelope"]["decision"] if value else None,
                              "run_status": value["envelope"]["runStatus"] if value else None}

            # Different task requests run concurrently against the same real source, with isolated outputs.
            with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
                for name, result in pool.map(execute, ("allow", "other_task")):
                    cases[name] = result
            for case in ("missing_approval", "expired_approval", "revoked_approval", "missing_specialist", "cancelled", "other_candidate"):
                name, result = execute(case)
                cases[name] = result
            after = tree_digest(repository)
            if before != after:
                raise ValueError("read-only composition changed source repository")
            verify_corpus(corpus)
            # Captured producer bytes in the working input directory are unchanged as well.
            for name, expected in provenance["artifacts"].items():
                if sha((inputs / name).read_bytes()) != expected:
                    raise ValueError("retained input artifact changed during composition")
            report = {"profile": "gitguard.local-five-producer-replay/v1", "production_authenticated": False,
                      "execution_authorized": False, "candidate_oid": provenance["candidate"], "base_oid": provenance["base"],
                      "retained_producer_pins": provenance["producerPins"], "retained_provenance_sha256": PROVENANCE_SHA256,
                      "current_consumer_binary_sha256": {name: tool.digest for name, tool in tools.items()},
                      "fresh_gitguard_stdout_sha256": sha((current / "stdout.json").read_bytes()),
                      "retained_specguard_decision": envelopes["specguard"]["decision"], "cases": cases,
                      "source_before": before, "source_after": after, "source_unchanged": before == after,
                      "limitations": ["retained actual producer captures, not reexecuted producers",
                                      "fixture authority, not production authentication", "no native parity rerun",
                                      "SG review approval freshness only; baseline authority not freshly revalidated",
                                      "concurrent mismatched task rejects; not two new fully evaluated task pipelines",
                                      "no synthetic-queue or late-completion scenario in this slice"]}
            write(output / "result.json", report)
            return report
    finally:
        for tool in tools.values():
            tool.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("gitguard", "gitguard-sha256", "flowguard", "flowguard-sha256", "output"):
        parser.add_argument("--" + name, required=True)
    options = parser.parse_args()
    try:
        result = run(options)
    except (ValueError, OSError, KeyError, subprocess.SubprocessError) as error:
        parser.exit(4, f"local composition rejected: {error}\n")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
