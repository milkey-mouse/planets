#!/bin/sh

find_cmd() {
    find assets -path assets/originals -prune -o -name '.*' -prune -o -not -type d -print
}

(
    echo '#![allow(irrefutable_let_patterns)]'
    echo '#![allow(non_upper_case_globals)]'
    echo '#![allow(dead_code)]'
    echo
    echo 'pub enum Asset {'
    find_cmd | rev | cut -d. -f1 | sort | uniq | rev \
    | sed "s/^\(.\)\(.*\)$/    \u\1\L\2\E(\&'static [u8]),/"
    echo '}'
    echo
    echo 'impl Asset {'
    find_cmd | rev | cut -d. -f1 | sort | uniq | rev | while read -r EXT; do
        [ -z "${EXT}" ] && continue
        EXT_TITLE_CASE="$(echo "${EXT}" | sed 's/^\(.\)\(.*\)$/\u\1\L\2/')"
        sed 's/^    //' <<EOF
        pub fn ${EXT}_data(&self) -> &'static [u8] {
            if let Asset::${EXT_TITLE_CASE}(data) = self {
                data
            } else {
                panic!("unwrapped asset as wrong file type");
            }
        }

EOF
    done | head -n-1
    echo '}'
    echo

    find_cmd | tr -c '.[:alnum:]\n' '_' | sort \
    | sed 's/^assets_\(.*\)\.\(.\)\(.*\)$/pub const \1: Asset = Asset::\u\2\L\3\E(include_bytes!("..\/assets\/\1.\2\3"));/'
) | tee src/assets.rs
