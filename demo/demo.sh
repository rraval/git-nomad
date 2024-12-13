#!/usr/bin/env bash
# shellcheck disable=SC2016,SC2215
set -euo pipefail

FAST=0
while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --fast)
            FAST=1
            shift
            ;;

        --trace)
            set -x
            shift
            ;;

        *)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

die() {
    echo "$@" >&2
    exit 1
}

assert_command_exists() {
    command -v "$1" >/dev/null || die "$1 not found"
}

assert_command_exists git-nomad
assert_command_exists pv
assert_command_exists setterm

fgDesktop=$(setterm --foreground cyan)
fgLaptop=$(setterm --foreground magenta)
fgReset=$(setterm --foreground default)

export GIT_NOMAD_USER=user
export GIT_NOMAD_HOST=

prompt() {
    case "${GIT_NOMAD_HOST}" in
        desktop)
            printf '%buser@desktop:~/repo$%b ' "${fgDesktop}" "${fgReset}"
            ;;

        laptop)
            printf '%buser@laptop:~/repo$%b ' "${fgLaptop}" "${fgReset}"
            ;;

        *)
            printf '$ '
            ;;
    esac
}

# https://github.com/sharkdp/fd/blob/d62bbbb/doc/screencast.sh#L28C1-L30C2
say() {
    if ((FAST)); then
        printf '%b\n' "$1"
    else
        printf '%b\n' "$1" | pv -qL $((20+(-2 + RANDOM%5)))
    fi
}

_() {
    say "# $1"
}

:() {
    prompt
    say "$1"

    if ! ((FAST)); then
        sleep 0.5
    fi

    eval "$1"
}

---() {
    echo

    if ! ((FAST)); then
        sleep 1
    fi
}

root=$(mktemp -d)
# $root is not re-assigned, it can be expanded now.
# shellcheck disable=SC2064
trap "rm -rf '${root}'" EXIT
cd "${root}"

export PAGER=
export GIT_AUTHOR_NAME=user
export GIT_AUTHOR_EMAIL=user@example.com
export GIT_COMMITTER_NAME=user
export GIT_COMMITTER_EMAIL=user@example.com

mkdir origin
(
    cd origin
    git init --bare --initial-branch=main
) >/dev/null 2>&1

(
    git clone "${root}/origin" desktop
    cd desktop
    git commit --allow-empty -m 'initial commit'
    git push -u origin HEAD:main
) >/dev/null 2>&1

(
    git clone "${root}/origin" laptop
) >/dev/null 2>&1

_ 'Install the `git-nomad` binary somewhere on your $PATH.'
_ 'Which lets you invoke `git nomad` as a subcommand of `git`.'
: "git nomad --version"

---

_ "This screencast simulates switching between ${fgDesktop}desktop${fgReset} and"
_ "${fgLaptop}laptop${fgReset} machines to show how git-nomad works."

---

_ "Let's start on the ${fgDesktop}desktop${fgReset} inside an empty git"
_ "repository on the \`main\` branch."

GIT_NOMAD_HOST=desktop
cd desktop

: "ls"
: "git branch"

---

_ "Let's make a branch and hack on some work-in-progress changes."
: "git checkout -b idea"
: "echo 'Start of an idea' > idea.txt"
: "git add idea.txt"
: "git commit -m 'initial idea'"

---

_ 'Run `git nomad sync` as often as you like'
: "git nomad sync"

---

_ "Make changes, freely mutate history."
: "echo 'Refining idea' >> idea.txt"
: "git commit -a --amend --no-edit"
: "git nomad sync"

---

_ "Now let's switch to the ${fgLaptop}laptop${fgReset}."
GIT_NOMAD_HOST=laptop
cd ../laptop

---

_ 'On the `main` branch there are no files'
: "ls"
: "git branch"

---

_ "Let's pick up branches from the \`desktop\`"
: "git nomad sync"

---

_ "Now we can create a local branch based off the desktop ref..."
: "git checkout -b idea refs/nomad/desktop/idea"

---

_ "... and use standard git workflows to pick up where we left off"
: "ls"
: "cat idea.txt"
: "echo 'Finalize idea' >> idea.txt"
: "git commit -a -m 'Finish idea on laptop'"

---

_ 'Continue running `git nomad sync` as often as you like'
: "git nomad sync"

---

_ "Back to the ${fgDesktop}desktop${fgReset}"
GIT_NOMAD_HOST=desktop
cd ../desktop
: "git nomad sync"

---

_ "These refs work with all the standard git commands."
_ "Let's see what the laptop commits changed."
: "git diff HEAD refs/nomad/laptop/idea"

---

_ "Let's see what happens when the desktop branch gets deleted"
: "git checkout main"
: "git branch -D idea"
: "git nomad sync"

---

_ "Note the \"Delete refs/nomad/desktop/idea\" line in the output"
_ "Let's switch to the ${fgLaptop}laptop${fgReset}..."
GIT_NOMAD_HOST=laptop
cd ../laptop

---

_ "... and run the sync"
: "git nomad sync"

---

_ "Note the \"Delete refs/nomad/desktop/idea\" line that cleans up the deleted branch"

---

_ "Under the hood, git-nomad pushes local branches to remote refs"
: "git ls-remote origin 'refs/nomad/*'"

---

_ "These remote refs are scoped under \`refs/nomad/<user>/*\`"
_ "to allow multiple people to simultaneously use git-nomad on"
_ "the same origin without interfering with each other."

---

_ "git-nomad will automatically clean up remote refs as local"
_ "branches get deleted. But you can also explicitly delete all"
_ "refs if you want to stop using git-nomad."
: "git nomad purge"

echo  # Need a extra newline for some reason
---

_ "You can even clean up refs for other hosts, which is handy"
_ "if you no longer have access to that machine."
: "git nomad purge --host desktop"

echo  # Need a extra newline for some reason
---

_ "Thanks for checking out git-nomad!"
