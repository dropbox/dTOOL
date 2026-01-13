#!/bin/bash
# BUG-130: References gnachman/DashTerm2 for PR testing (development tool).
[ $# -ne 1 ] && echo Usage: test_pull number && exit
git checkout master && git branch -D pull$1
git branch pull$1 && git checkout pull$1 && curl https://github.com/gnachman/DashTerm2/pull/$1.patch | git am --ignore-space-change --ignore-whitespace
