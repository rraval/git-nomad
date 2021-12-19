export GIT_AUTHOR_NAME=user
export GIT_AUTHOR_EMAIL=user@example.com
export GIT_COMMITTER_NAME=user
export GIT_COMMITTER_EMAIL=user@example.com

export PS1='\$ '

host() {
    echo
    figlet -c $1
    export PS1=$1':\w\$ '
    echo

    mkdir -p /tmp/$1
    cd /tmp/$1
    export HOME=/tmp/$1
    export GIT_NOMAD_HOST=$1
}

if [[ -n "$HOST" ]]; then
    host "$HOST"
fi
