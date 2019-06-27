use sample::signal;

use super::{sink::Sink, CanonicalSignal};
use crate::assets;

pub fn vlem(sink: &dyn Sink) -> impl CanonicalSignal {
    signal::from_iter(
        sink.load(&assets::vlem0)
            .into_interleaved_iterator()
            .chain(sink.load(&assets::vlem1).into_interleaved_iterator())
            .chain(sink.load(&assets::vlem2).into_interleaved_iterator())
            .chain(sink.load(&assets::vlem3).into_interleaved_iterator())
            .chain(sink.load(&assets::vlem4).into_interleaved_iterator())
            .chain(sink.load(&assets::vlem5).into_interleaved_iterator())
            .chain(sink.load(&assets::vlem6).into_interleaved_iterator())
            .chain(sink.load(&assets::vlem7).into_interleaved_iterator()),
    )
}
