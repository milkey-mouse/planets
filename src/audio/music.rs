use sample::signal;

use super::{source, CanonicalSignal};
use crate::assets;

/*struct LoadableSource {
    data: &'static [u8],
    source: Option<Box<dyn CanonicalSignal>>,
}

impl LoadableSource {
    fn load()  {

    }
}*/

pub fn vlem() -> impl CanonicalSignal {
    signal::from_iter(
        source::new(assets::vlem0)
            .until_exhausted()
            .chain(source::new(assets::vlem1).until_exhausted())
            .chain(source::new(assets::vlem2).until_exhausted())
            .chain(source::new(assets::vlem3).until_exhausted())
            .chain(source::new(assets::vlem4).until_exhausted())
            .chain(source::new(assets::vlem5).until_exhausted())
            .chain(source::new(assets::vlem6).until_exhausted())
            .chain(source::new(assets::vlem7).until_exhausted()),
    )
}
