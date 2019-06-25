// TODO: prefer() with references

// TODO: prefer() should be a wrapper around prefer_fn()
pub fn prefer<'a, T: PartialEq + 'a>(
    wanted: impl IntoIterator<Item = &'a T>,
    supported: impl IntoIterator<Item = T>,
    default_to_first: bool,
) -> Option<T> {
    // NOTE: if T were guaranteed to support Hash, we could use a HashSet here
    let wanted = wanted.into_iter().collect::<Vec<_>>();
    let mut supported = supported.into_iter();

    let first = match supported.next() {
        Some(x) => x,
        None => return None,
    };

    if wanted.contains(&&first) {
        Some(first)
    } else {
        supported
            .find(|x| wanted.contains(&x))
            .or_else(|| Some(first).filter(|_| default_to_first))
    }
}

pub fn prefer_fn<'a, T: 'a>(
    wanted: impl Fn(&T) -> bool,
    supported: impl IntoIterator<Item = T>,
    default_to_first: bool,
) -> Option<T> {
    let mut supported = supported.into_iter();

    let first = match supported.next() {
        Some(x) => x,
        None => return None,
    };

    if wanted(&first) {
        Some(first)
    } else {
        supported
            .find(wanted)
            .or_else(|| Some(first).filter(|_| default_to_first))
    }
}
