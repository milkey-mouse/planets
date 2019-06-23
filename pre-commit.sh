#!/bin/sh
set -e

./generate-shader-macros.sh 1>/dev/null
./generate-asset-macros.sh 1>/dev/null
git add src/render/shaders.rs src/assets.rs
