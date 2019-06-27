use sample::{
    frame::{Frame, Mono},
    signal::{self, Signal},
};

mod mix;
mod music;
mod sink;
mod source;

const HIGH_QUALITY_INTERPOLATION: bool = true;

// this probably would be i16 were it not for Interpolators requiring f64 frames
pub type CanonicalSample = f64;

// TODO: CanonicalSignal should be a trait alias once stabilized
// See https://github.com/rust-lang/rust/issues/41517.

type CanonicalFrame = Mono<CanonicalSample>;

trait CanonicalSignal {
    fn into_interleaved_iterator(
        self: Box<Self>,
    ) -> Box<dyn Iterator<Item = CanonicalFrame> + Send + Sync>;
    fn into_interleaved_signal(
        self: Box<Self>,
    ) -> Box<dyn Signal<Frame = CanonicalFrame> + Send + Sync>;
}

impl<T, F> CanonicalSignal for T
where
    T: Signal<Frame = F> + Send + Sync + 'static,
    F: Frame<Sample = CanonicalSample> + 'static,
    F::Channels: Send + Sync + 'static,
{
    fn into_interleaved_iterator(
        self: Box<Self>,
    ) -> impl Iterator<Item = CanonicalFrame> + Send + Sync {
        // TODO: is .flat_map() unstable?
        self.until_exhausted().flat_map(F::channels).map(|s| [s])
    }

    fn into_interleaved_signal(
        self: Box<Self>,
    ) -> impl Signal<Frame = CanonicalFrame> + Send + Sync {
        signal::from_iter(
            self.until_exhausted()
                .flat_map(Frame::channels)
                .map(|s| [s]),
        )
    }
}
