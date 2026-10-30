#!/usr/bin/env bash
# Disposable Linux CI only. Never replace the host's /usr/bin/git.
set -euo pipefail
mode=${1:?build or run}
shift
case "$mode" in
  build)
    destination=${1:?fresh private build directory}
    test ! -e "$destination"
    mkdir -m 700 "$destination"
    destination=$(realpath "$destination")
    archive="$destination/git-2.47.3.tar.xz"
    curl --fail --show-error --silent --location --proto '=https' --tlsv1.2 \
      https://www.kernel.org/pub/software/scm/git/git-2.47.3.tar.xz -o "$archive"
    # Official kernel.org release checksum, verified before adding this pin.
    printf '%s  %s\n' 9c2eb1250781b3e5bfef098572d07fdf132d67e6c065e4307332ade9819a1501 "$archive" | sha256sum --check -
    python3 - "$archive" "$destination" <<'PY'
import pathlib, sys, tarfile
with tarfile.open(sys.argv[1]) as archive:
    members = archive.getmembers()
    assert len(members) < 10000
    assert sum(m.size for m in members) < 100 * 1024 * 1024
    for member in members:
        path = pathlib.PurePosixPath(member.name)
        assert not path.is_absolute() and '..' not in path.parts
        assert path.parts[0] == 'git-2.47.3'
        if member.issym():
            assert member.name == 'git-2.47.3/RelNotes'
            assert member.linkname == 'Documentation/RelNotes/2.47.3.txt'
        else:
            assert member.isfile() or member.isdir()
    archive.extractall(sys.argv[2], filter='data')
PY
    make -C "$destination/git-2.47.3" -j2 prefix="$destination/install" \
      NO_GETTEXT=YesPlease NO_TCLTK=YesPlease NO_PERL=YesPlease \
      NO_CURL=YesPlease NO_EXPAT=YesPlease all install
    test "$("$destination/install/bin/git" --version)" = 'git version 2.47.3'
    ;;
  run)
    if test "${1:-}" = system; then
      shift
      exec "$@"
    fi
    export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
    export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
    binary=$(realpath "${1:?built private Git binary}")
    shift
    test "$#" -gt 0
    test "$("$binary" --version)" = 'git version 2.47.3'
    before=$(sha256sum /usr/bin/git)
    # Mount changes are private to this child namespace. Drop back to the
    # invoking CI user before running any repository code or test fixture.
    status=0
    sudo --preserve-env=HOME,PATH,CARGO_HOME,RUSTUP_HOME,CARGO_TARGET_DIR,CARGO_BUILD_JOBS,CARGO_INCREMENTAL,CARGO_PROFILE_TEST_DEBUG,CARGO_PROFILE_DEV_DEBUG \
      unshare --mount --propagation private --fork \
      /bin/bash -c '
        set -euo pipefail
        binary=$1; user=$2; shift 2
        mount --bind "$binary" /usr/bin/git
        test "$(/usr/bin/git --version)" = "git version 2.47.3"
        exec runuser --preserve-environment -u "$user" -- "$@"
      ' ci-git-minimum "$binary" "$(id -un)" "$@" || status=$?
    test "$(sha256sum /usr/bin/git)" = "$before"
    exit "$status"
    ;;
  *) echo 'expected build or run' >&2; exit 2 ;;
esac
