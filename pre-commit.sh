#!/bin/sh
set -e

#cargo fmt -- --force --write-mode diff
./generate-shader-macros.sh 1>/dev/null
git add src/shaders.rs
