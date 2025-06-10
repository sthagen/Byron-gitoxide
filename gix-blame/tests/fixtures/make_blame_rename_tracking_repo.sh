#!/usr/bin/env bash
set -eu -o pipefail

git init -q
git config --local diff.algorithm histogram

git config merge.ff false

git checkout -q -b main

seq 1 4 > before-rename.txt
git add before-rename.txt
git commit -q -m c1

mv before-rename.txt after-rename.txt
git add before-rename.txt after-rename.txt
git commit -q -m c2

seq 1 5 > after-rename.txt
git add after-rename.txt
git commit -q -m c3

git checkout -b different-branch
git reset --hard HEAD~2

seq 0 4 > before-rename.txt
git add before-rename.txt
git commit -q -m c10

mv before-rename.txt after-rename.txt
git add before-rename.txt after-rename.txt
git commit -q -m c11

git checkout main
git merge different-branch || true

git blame --porcelain after-rename.txt > .git/after-rename.baseline
