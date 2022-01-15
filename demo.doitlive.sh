#doitlive env: GIT_AUTHOR_NAME=user
#doitlive env: GIT_AUTHOR_EMAIL=user@example.com
#doitlive env: GIT_COMMITTER_NAME=user
#doitlive env: GIT_COMMITTER_EMAIL=user@example.com
#doitlive prompt: $

#You must have the `git-nomad` binary on your PATH
which git-nomad

#
#Now you can simply invoke it as a `git` subcommand
git nomad --version

#doitlive env: GIT_NOMAD_HOST=desktop
#
#This demo jumps between multiple different machine.
#
#     _           _    _
#  __| | ___  ___| | _| |_ ___  _ __
# / _` |/ _ \/ __| |/ / __/ _ \| '_ \
#| (_| |  __/\__ \   <| || (_) | |_) |
# \__,_|\___||___/_|\_\\__\___/| .__/
#                              |_|
#
#Let's pretend we're on a desktop machine and clone a repo.
cd /tmp/desktop
#doitlive prompt: {cwd}:{vcs_branch}$
git clone git@github.com:rraval/workflows
cd workflows

#
#Let's hack on some code on a branch
git checkout -b feature

echo line1 > myfile
git add .
git commit -m 'added a line'

#
#Run `git nomad sync` as often as you like!
git nomad sync

#
#Make changes, freely mutate history
#We don't care about fast forwards
echo line2 >> myfile
git commit -a --amend --no-edit
git nomad sync

#doitlive env: GIT_NOMAD_HOST=laptop
#
# _             _
#| | __ _ _ __ | |_ ___  _ __
#| |/ _` | '_ \| __/ _ \| '_ \
#| | (_| | |_) | || (_) | |_) |
#|_|\__,_| .__/ \__\___/| .__/
#        |_|            |_|
#
#Now we're on the road with the laptop
cd /tmp/laptop

#
#Clone the repo as usual
git clone git@github.com:rraval/workflows
cd workflows

#
#Pick up our branch where we left off.
git nomad sync

#
#We can simply create a branch off the desktop ref
git checkout -b feature refs/nomad/desktop/feature
cat myfile
echo line3 >> myfile
git commit -a -m 'added lines from laptop'

#
#We can sync freely on the laptop side as well
git nomad sync

#doitlive env: GIT_NOMAD_HOST=desktop
#
#     _           _    _
#  __| | ___  ___| | _| |_ ___  _ __
# / _` |/ _ \/ __| |/ / __/ _ \| '_ \
#| (_| |  __/\__ \   <| || (_) | |_) |
# \__,_|\___||___/_|\_\\__\___/| .__/
#                              |_|
#
#Back on the desktop
cd /tmp/desktop/workflows

#
#We don't need this feature branch anymore, the laptop branch has newer changes
git branch
git checkout master
git branch -D feature

#
#Syncing will notice the deleted branch and delete the nomad ref
git nomad sync

#doitlive env: GIT_NOMAD_HOST=laptop
#
# _             _
#| | __ _ _ __ | |_ ___  _ __
#| |/ _` | '_ \| __/ _ \| '_ \
#| | (_| | |_) | || (_) | |_) |
#|_|\__,_| .__/ \__\___/| .__/
#        |_|            |_|
#
#Now on the laptop
cd /tmp/laptop/workflows

#
#Syncing notices the desktop nomad ref is gone and deletes it
git nomad sync

#
#You can stop using `git-nomad` at any time.
#
#Clean up nomad refs for a given host (even a different host!)
git nomad purge --host desktop

#Or clean up all traces of `git-nomad`
git nomad purge --all
