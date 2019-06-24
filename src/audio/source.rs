use hound::{SampleFormat, WavReader};
use lewton::inside_ogg::OggStreamReader;
use sample::{
    conv::ToSample,
    frame::{Mono, Stereo},
    signal::{self, Signal},
    Sample,
};

use std::{io::Cursor, iter};

use super::{resample, CanonicalFormat, CanonicalSignal};
use crate::assets::Asset;

pub fn new(asset: Asset) -> Box<CanonicalSignal> {
    match asset {
        Asset::Wav(data) => {
            let reader = WavReader::new(Cursor::new(data)).unwrap();

            match reader.spec().sample_format {
                SampleFormat::Float => create_signal(
                    reader.spec().sample_rate,
                    reader.spec().channels,
                    reader.into_samples::<f32>().filter_map(Result::ok),
                ),
                SampleFormat::Int => match reader.spec().bits_per_sample {
                    0..=16 => create_signal(
                        reader.spec().sample_rate,
                        reader.spec().channels,
                        reader.into_samples::<i16>().filter_map(Result::ok),
                    ),
                    16..=32 => create_signal(
                        reader.spec().sample_rate,
                        reader.spec().channels,
                        reader.into_samples::<i32>().filter_map(Result::ok),
                    ),
                    _ => panic!(),
                },
            }
        }
        Asset::Ogg(data) => {
            let mut reader = OggStreamReader::new(Cursor::new(data)).unwrap();

            create_signal(
                reader.ident_hdr.audio_sample_rate,
                reader.ident_hdr.audio_channels.into(),
                iter::from_fn(move || reader.read_dec_packet_itl().unwrap())
                    .fuse() // TODO: is this fuse() redundant?
                    .flatten(),
            )
        }
        _ => panic!("tried to load sound asset of unknown type"),
    }
}

fn create_signal<I, S>(sample_rate: u32, channels: u16, iterator: I) -> Box<CanonicalSignal>
where
    I: Iterator<Item = S> + Sync + Send + 'static,
    S: Sample + ToSample<CanonicalFormat> + 'static,
{
    let iterator = iterator.into_iter().map(S::to_sample);

    let signal: Box<CanonicalSignal> = match channels {
        1 => Box::new(
            signal::from_interleaved_samples_iter::<_, Mono<_>>(iterator).map(|f| [f[0], f[0]]),
        ),
        2 => Box::new(signal::from_interleaved_samples_iter::<_, Stereo<_>>(
            iterator,
        )),
        _ => panic!("too many channels"),
    };

    resample(sample_rate, None, signal)
}
