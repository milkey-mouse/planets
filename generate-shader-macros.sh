#!/bin/sh

# TODO: there should be some sort of way to run this sort of build script as a
# Cargo task or something (if it were written in Rust)

for VTYPE in vertex fragment; do
    EXTENSION="$(echo ${VTYPE} | cut -c-4)"
    find shaders -type f -name "*.${EXTENSION}" | while read -r VPATH; do
        VNAME="$(basename "${VPATH}" ".${EXTENSION}")"
        sed 's/^    //' <<EOF
        pub mod ${VNAME}_${EXTENSION} {
            vulkano_shaders::shader!{
                ty: "${VTYPE}",
                path: "${VPATH}"
            }
EOF
        if [ "${VTYPE}" != "fragment" ]; then
            IN_VARS="$(sed -nE 's/^ *layout ?\(location = ([0-9]*)\) in ([0-9a-z]*)  *([a-z]*);.*$/\1 \2 \3/p' "${VPATH}")"
            if [ -n "${IN_VARS}" ]; then
                echo
                echo '        #[derive(Debug, Clone, Default)]'
                echo '        pub struct Vertex {'
                echo "${IN_VARS}" | while IFS=" " read LOCATION TYPE NAME; do
                    RUST_TYPE="$(echo "${TYPE}" | sed -E 's/^vec([1-4])$/\[f32; \1\]/;s/^float$/f32/')"
                    echo "            pub ${NAME}: ${RUST_TYPE},"
                done
                echo '        }'
                ALL_VARS="$(echo "${IN_VARS}" | cut -d' ' -f3 | tr '\n' ' ' | sed 's/ $//;s/ /, /g')"
                echo "        vulkano::impl_vertex!(Vertex, ${ALL_VARS});"
            fi
        fi
        echo "    }"
        echo
    done
done | head -n-1 | if [ "$1" == "--mod" ]; then
    echo 'mod shaders {'
    cat
    echo '}'
else
    sed 's/^    //' | tee src/shaders.rs
fi
