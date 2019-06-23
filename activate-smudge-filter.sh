#!/bin/sh

git config filter.rustfmt.clean 'rustfmt-9999 || rustfmt || cat'
git config filter.rustfmt.smudge cat
