#!/bin/sh

# TODO: there should be some sort of way to run this sort of build script as a
# Cargo task or something (if it were written in Rust)

for VTYPE in vertex fragment; do
    EXTENSION="$(echo ${VTYPE} | cut -c-4)"
    find shaders -type f -name "*.${EXTENSION}" | while read -r VPATH; do
        VNAME="$(basename "${VPATH}" ".${EXTENSION}")"
        sed 's/^    //' <<EOF
        pub mod ${VNAME} {
            vulkano_shaders::shader!{
                ty: "${VTYPE}",
                path: "${VPATH}"
            }
        }

EOF
    done
done | head -n-1 | if [ "$1" == "--mod" ]; then
    echo 'mod shaders {'
    cat
    echo '}'
else
    sed 's/^    //' | tee src/shaders.rs
fi
