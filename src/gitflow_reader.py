"""只读适配现有 GitFlow 加载器；嵌入 Rust 二进制，不另建策略引擎。"""
import json
import os
from pathlib import Path
import subprocess
import sys
from threading import Thread

# 两个平台都禁止 status 的可选索引刷新写入。
os.environ['GIT_OPTIONAL_LOCKS'] = '0'
sys.path.insert(0, str(Path(sys.argv[1]) / 'scripts'))
from gitflow import git

# GitFlow 原传输依赖 POSIX selector；Windows 仅替换 Git 只读进程传输。
# 规则校验、激活摘要、组织继承、worktree 定位与漂移判定仍使用原加载器。
if os.name == 'nt':
    def platform_run(path, *args, check=True, timeout=10):
        if not args or args[0] not in {'rev-parse', 'status', 'symbolic-ref', 'show-ref', 'for-each-ref', 'config', 'worktree', 'remote', 'check-ref-format', 'var'}:
            raise git.FlowError('adapter_read_only', '不允许适配器执行写命令。')
        if args[0] == 'config' and '--get' not in args and '--get-regexp' not in args:
            raise git.FlowError('adapter_read_only', '只允许读取 Git 配置。')
        if args[0] == 'worktree' and (len(args) < 2 or args[1] != 'list'):
            raise git.FlowError('adapter_read_only', '只允许读取 worktree 清单。')
        if args[0] == 'remote' and len(args) != 1:
            raise git.FlowError('adapter_read_only', '只允许读取 remote 名称。')
        # 仓库/配置路由不能由环境覆盖，但 AUTHOR/COMMITTER 身份须与原生提交一致。
        routing = {'GIT_DIR', 'GIT_WORK_TREE', 'GIT_COMMON_DIR', 'GIT_INDEX_FILE',
                   'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES',
                   'GIT_NAMESPACE', 'GIT_SHALLOW_FILE', 'GIT_CEILING_DIRECTORIES',
                   'GIT_DISCOVERY_ACROSS_FILESYSTEM'}
        env = {k:v for k,v in os.environ.items()
               if k not in routing and not k.startswith('GIT_CONFIG_')}
        env['GIT_TERMINAL_PROMPT'] = '0'
        env['GIT_OPTIONAL_LOCKS'] = '0'
        try:
            process = subprocess.Popen(['git', '-c', 'core.fsmonitor=false', '-C', str(path), *args], stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
            chunks = [b'', b'']
            def read_stream(stream, index):
                chunks[index] = stream.read(4*1024*1024+1)
                if len(chunks[index]) > 4*1024*1024:
                    process.kill()
                stream.close()
            readers = [Thread(target=read_stream, args=(process.stdout, 0)), Thread(target=read_stream, args=(process.stderr, 1))]
            for reader in readers:
                reader.start()
            try:
                code = process.wait(timeout=min(timeout,10))
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait()
                for reader in readers:
                    reader.join()
            result = subprocess.CompletedProcess(process.args, code, *chunks)
        except (OSError, subprocess.TimeoutExpired) as exc:
            raise git.FlowError('git_unavailable', 'Git 读取失败或超时。') from exc
        if len(result.stdout)+len(result.stderr)>4*1024*1024:
            raise git.FlowError('git_output_limit', 'Git 输出超过限制。')
        if check and result.returncode:
            raise git.FlowError('git_failed', 'Git 只读查询失败。')
        if result.returncode == 0 and args == ('rev-parse', '--show-toplevel'):
            result.stdout = (str(Path(result.stdout.decode().strip()).resolve()) + '\n').encode()
        return result
else:
    original_run = git.run
    def platform_run(path, *args, **kwargs):
        return original_run(path, '-c', 'core.fsmonitor=false', *args, **kwargs)


def read_git(path, *args, **kwargs):
    # git status 可依据 .gitattributes + filter.* 执行 clean/process 命令。
    # 在任何 status 之前先读取有效配置；不能只关闭 fsmonitor 或伪造干净状态。
    if args and args[0] == 'status':
        configured = platform_run(path, 'config', '--null', '--get-regexp', r'^filter\..*\.(clean|process)$', check=False)
        if configured.returncode not in (0, 1):
            raise git.FlowError('filter_configuration_unverified', '无法安全核验项目 filter 配置。')
        # --get-regexp 按 Git 配置优先级返回全部值；相同 key 最后一个值生效。
        # 使用 NUL 分隔，避免值内换行或空值改变可执行配置判断。
        effective = {}
        for entry in configured.stdout.split(b'\0'):
            if not entry:
                continue
            key, separator, value = entry.partition(b'\n')
            if not separator:
                raise git.FlowError('filter_configuration_unverified', 'filter 配置输出无法可靠解析。')
            effective[key] = value
        if any(value.strip() for value in effective.values()):
            raise git.FlowError('executable_filter_unverified', '项目存在有效 clean/process filter 命令，无法在只读边界内核验状态；需隔离或明确授权后复验。')
    return platform_run(path, *args, **kwargs)


git.run = read_git

try:
    from gitflow.policy import load
    facts = git.require_repo(sys.argv[2])
    policy, active = load(facts)
    result = {'status':'active', 'source':'gitflow-plugin active policy',
              'profile':policy['profile'], 'revision':policy['revision'],
              'policy_sha256':active['sha256'], 'roles':policy['roles'],
              'primary_remote':policy['primary_remote'], 'merge_method':policy['merge_method'],
              'commit_message_pattern':policy['commit_message_pattern'],
              'collaboration':policy['collaboration'], 'quality_decision':'not_evaluated'}
    if len(sys.argv) > 3 and sys.argv[3]:
        request = json.loads(sys.argv[3])
        if request['action'] == 'branch':
            from gitflow.policy import role_for
            from gitflow.git import valid_branch, report, reason
            branch = request['branch']
            role = role_for(policy, branch) if valid_branch(facts['root'], branch) else None
            result = report('policy.branch', 'allow' if role else 'deny',
                            [] if role else [reason('branch_name_unmatched', '分支名称不符合激活策略角色。')],
                            branch=branch, role=role, revision=policy['revision'], policy_sha256=active['sha256'],
                            quality_decision='not_evaluated')
        else:
            from gitflow.service import gate
            result = gate(sys.argv[2], request['action'], source=request.get('source'),
                          target=request.get('target'), message=request.get('message'))
except git.FlowError as exc:
    result = {'status':'unverified','code':exc.code,'reason':exc.message,'quality_decision':'not_evaluated'}
except Exception:
    result = {'status':'unverified','reason':'Adapter version or state is unsupported','quality_decision':'not_evaluated'}
if len(sys.argv) > 3 and sys.argv[3] and 'decision' not in result:
    result['decision'] = 'unverified'
print(json.dumps(result,ensure_ascii=False))
