use super::{sink::Sink, source::Source};
use crate::assets::{self, Asset};

pub fn vlem<'a>(sink: &dyn Sink) -> Source<'a> {
    const VLEM: [&Asset; 8] = [
        &assets::vlem0,
        &assets::vlem1,
        &assets::vlem2,
        &assets::vlem3,
        &assets::vlem4,
        &assets::vlem5,
        &assets::vlem6,
        &assets::vlem7,
    ];

    VLEM[1..]
        .iter()
        .map(|&a| Source::new(a))
        .fold(Source::new(VLEM[0]), Source::chain)
        .canonicalize(sink)
}
