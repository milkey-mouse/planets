use hound::{SampleFormat, WavReader};
use lewton::inside_ogg::OggStreamReader;
use sample::{
    conv::ToSample,
    frame::{Mono, Stereo},
    signal::{self, Signal},
    Sample,
};

use std::{io::Cursor, iter};

use super::{
    sink::{RealSink, Sink},
    CanonicalSample, CanonicalSignal,
};
use crate::assets::Asset;

pub fn new(asset: &'static Asset, sink: &RealSink) -> Box<dyn CanonicalSignal + Send + Sync> {
    match asset {
        Asset::Wav(data) => {
            let reader = WavReader::new(Cursor::new(data)).unwrap();

            match reader.spec().sample_format {
                SampleFormat::Float => create_signal(
                    sink,
                    reader.spec().sample_rate,
                    reader.spec().channels,
                    reader.into_samples::<f32>().filter_map(Result::ok),
                ),
                SampleFormat::Int => match reader.spec().bits_per_sample {
                    0...16 => create_signal(
                        sink,
                        reader.spec().sample_rate,
                        reader.spec().channels,
                        reader.into_samples::<i16>().filter_map(Result::ok),
                    ),
                    17...32 => create_signal(
                        sink,
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
                sink,
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

fn create_signal<I, S>(
    sink: &RealSink,
    in_sample_rate: u32,
    channels: u16,
    iterator: I,
) -> Box<dyn CanonicalSignal + Send + Sync>
where
    I: IntoIterator<Item = S>,
    I::IntoIter: Sync + Send + 'static,
    S: Sample + ToSample<CanonicalSample> + 'static,
{
    let iterator = iterator.into_iter().map(S::to_sample);

    match (channels, sink.channels().unwrap()) {
        (1, 1) => sink.sample_rate().resample_from(
            in_sample_rate,
            signal::from_interleaved_samples_iter::<_, Mono<_>>(iterator),
        ),
        (1, 2) => sink.sample_rate().resample_from(
            in_sample_rate,
            signal::from_interleaved_samples_iter::<_, Mono<_>>(iterator).map(|f| [f[0], f[0]]),
        ),
        (2, 1) => sink.sample_rate().resample_from(
            in_sample_rate,
            signal::from_interleaved_samples_iter::<_, Stereo<_>>(iterator)
                .map(|f| [f[0].add_amp(f[1])]),
        ),
        (2, 2) => sink.sample_rate().resample_from(
            in_sample_rate,
            signal::from_interleaved_samples_iter::<_, Stereo<_>>(iterator),
        ),
        _ => panic!("too many channels"),
    }
}
