use hound::{self, WavReader};
use lewton::{inside_ogg::OggStreamReader, samples::InterleavedSamples};
use sample::{
    frame::{Frame, Mono, Stereo},
    interpolate::{self, Converter, Interpolator},
    ring_buffer,
    signal::{self, FromInterleavedSamplesIterator, IntoInterleavedSamplesIterator, Signal},
    Sample,
};

use std::{convert::TryInto, io::Cursor, num::NonZeroU32, vec};

use super::{sink::Sink, Channels, SampleFormat, HIGH_QUALITY_INTERPOLATION};
use crate::assets::Asset;

const SINC_BUFFER_SIZE: usize = 100;

type SourceResampler<T, F, I> =
    IntoInterleavedSamplesIterator<Converter<FromInterleavedSamplesIterator<T, F>, I>>;
enum Resampler<'a, F: Frame<Sample = SampleFormat>> {
    Linear(SourceResampler<Box<Source<'a>>, F, interpolate::Linear<F>>),
    Sinc(SourceResampler<Box<Source<'a>>, F, interpolate::Sinc<[F; SINC_BUFFER_SIZE]>>),
}

// TODO: should SourceReader be a trait?
// clippy seems to think so.
// the current enum variants would be separate structs implementing the trait,
// and boxed versions would be passed around.
// most things here are boxed anyway, so it wouldn't be too much perf loss
// and of course, this is all premature optimization because I've never seen
// the audio thread take more than 10% CPU, even on debug mode.
enum SourceReader<'a> {
    Wav(WavReader<Cursor<&'a [u8]>>),
    Ogg(
        OggStreamReader<Cursor<&'a [u8]>>,
        Option<vec::IntoIter<f32>>,
    ),

    Iterator(Box<dyn Iterator<Item = SampleFormat> + Send + Sync + 'a>),

    MonoResampler(Resampler<'a, Mono<SampleFormat>>),
    StereoResampler(Resampler<'a, Stereo<SampleFormat>>),

    MonoToStereo(Box<Source<'a>>, Option<SampleFormat>),
    StereoToMono(Box<Source<'a>>),
}

pub struct Source<'a> {
    reader: SourceReader<'a>,
    sample_rate: u32,
    channels: Channels,
}

impl<'a> Source<'a> {
    pub fn new(asset: &'a Asset) -> Self {
        match asset {
            Asset::Wav(data) => Self::from_wav(data),
            Asset::Ogg(data) => Self::from_ogg(data),
            _ => unreachable!(),
        }
    }

    fn from_wav(data: &'a [u8]) -> Self {
        let reader = WavReader::new(Cursor::new(data)).unwrap();
        let sample_rate = reader.spec().sample_rate;
        let channels = reader.spec().channels.try_into().unwrap();

        Self {
            reader: SourceReader::Wav(reader),
            sample_rate,
            channels,
        }
    }

    fn from_ogg(data: &'a [u8]) -> Self {
        let mut reader = OggStreamReader::new(Cursor::new(data)).unwrap();
        let chunk: vec::IntoIter<f32> = reader
            .read_dec_packet_generic::<InterleavedSamples<f32>>()
            .unwrap()
            .unwrap()
            .samples
            .into_iter();

        let sample_rate = reader.ident_hdr.audio_sample_rate;
        let channels = reader.ident_hdr.audio_channels.try_into().unwrap();

        Self {
            reader: SourceReader::Ogg(reader, Some(chunk)),
            sample_rate,
            channels,
        }
    }

    pub fn from_iterator<'b, I>(iterator: I, sample_rate: u32, channels: Channels) -> Self
    where
        I: Iterator<Item = SampleFormat> + Send + Sync + 'a,
        //I::IntoIter: Send + Sync + 'b,
    {
        Self {
            reader: SourceReader::Iterator(Box::new(iterator)),
            sample_rate,
            channels,
        }
    }

    pub fn chain(self, other: Source<'a>) -> Self {
        assert!(self.sample_rate == other.sample_rate);
        assert!(self.channels == other.channels);

        let sample_rate = self.sample_rate;
        let channels = self.channels;

        Self::from_iterator(Iterator::chain(self, other), sample_rate, channels)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> Channels {
        self.channels
    }

    pub fn canonicalize(self, sink: &dyn Sink) -> Self {
        if sink.channels().map(|c| self.channels > c).unwrap_or(false) {
            // resampling is an expensive operation, so if this source will be
            // mixed down to fewer channels, do that before resampling
            self.with_channels(sink.channels())
                .with_sample_rate(sink.sample_rate())
        } else {
            // on the other hand, if channels are being duplicated (e.g. mono
            // to stereo) we should resample first
            self.with_sample_rate(sink.sample_rate())
                .with_channels(sink.channels())
        }
    }

    pub fn with_channels<T: TryInto<Channels>>(self, channels: Option<T>) -> Self {
        use Channels::*;

        let sample_rate = self.sample_rate;

        if let Some(channels) = channels.and_then(|c| c.try_into().ok()) {
            match (self.channels, channels) {
                (Stereo, Stereo) | (Mono, Mono) => self,
                (Mono, Stereo) => Self {
                    reader: SourceReader::MonoToStereo(Box::new(self), None),
                    sample_rate,
                    channels,
                },
                (Stereo, Mono) => Self {
                    reader: SourceReader::StereoToMono(Box::new(self)),
                    sample_rate,
                    channels,
                },
            }
        } else {
            self
        }
    }

    pub fn with_sample_rate<T: TryInto<NonZeroU32>>(self, sample_rate: Option<T>) -> Self {
        let channels = self.channels;

        match sample_rate.and_then(|c| c.try_into().ok()) {
            Some(sample_rate) if self.sample_rate != sample_rate.get() => Self {
                reader: match self.channels {
                    Channels::Mono => SourceReader::MonoResampler(self.into_resampler(sample_rate)),
                    Channels::Stereo => {
                        SourceReader::StereoResampler(self.into_resampler(sample_rate))
                    }
                },
                sample_rate: sample_rate.get(),
                channels,
            },
            _ => self,
        }
    }

    fn into_resampler<F: Frame<Sample = SampleFormat>>(
        mut self,
        sample_rate: NonZeroU32,
    ) -> Resampler<'a, F> {
        if HIGH_QUALITY_INTERPOLATION {
            let buffer = ring_buffer::Fixed::from([F::equilibrium(); SINC_BUFFER_SIZE]);

            Resampler::Sinc(
                self.resample_with_interpolator(sample_rate, interpolate::Sinc::new(buffer)),
            )
        } else {
            let left = F::from_samples(&mut self).unwrap();
            let right = F::from_samples(&mut self).unwrap();

            Resampler::Linear(
                self.resample_with_interpolator(sample_rate, interpolate::Linear::new(left, right)),
            )
        }
    }

    fn resample_with_interpolator<F: Frame<Sample = SampleFormat>, I: Interpolator<Frame = F>>(
        self,
        new_sample_rate: NonZeroU32,
        interpolator: I,
    ) -> SourceResampler<Box<Source<'a>>, F, I> {
        let old_sample_rate = self.sample_rate;

        signal::from_interleaved_samples_iter(Box::new(self))
            .from_hz_to_hz(
                interpolator,
                old_sample_rate.into(),
                new_sample_rate.get().into(),
            )
            .into_interleaved_samples()
            .into_iter()
    }
}

impl<'a> Iterator for Source<'a> {
    type Item = SampleFormat;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.reader {
            SourceReader::Wav(reader) => match reader.spec().sample_format {
                hound::SampleFormat::Float => reader
                    .samples::<f32>()
                    .next()
                    .transpose()
                    .unwrap()
                    .map(Sample::to_sample),
                hound::SampleFormat::Int => match reader.spec().bits_per_sample {
                    0...16 => reader
                        .samples::<i16>()
                        .next()
                        .transpose()
                        .unwrap()
                        .map(Sample::to_sample),
                    17...32 => reader
                        .samples::<i32>()
                        .next()
                        .transpose()
                        .unwrap()
                        .map(Sample::to_sample),
                    _ => unreachable!(),
                },
            },
            // TODO: fork lewton to output to a &mut [f32]
            // or at least reuse its vector. there's lots of unnecessary allocations
            SourceReader::Ogg(reader, chunk) => chunk
                .as_mut()
                .and_then(Iterator::next)
                .or_else(|| {
                    *chunk = reader
                        .read_dec_packet_generic()
                        .unwrap()
                        .map(|s: InterleavedSamples<f32>| s.samples.into_iter());
                    chunk.as_mut().and_then(Iterator::next)
                })
                .map(Sample::to_sample),
            SourceReader::Iterator(iterator) => iterator.next(),
            SourceReader::MonoResampler(resampler) => match resampler {
                Resampler::Linear(linear) => linear.next(),
                Resampler::Sinc(sinc) => sinc.next(),
            },
            SourceReader::StereoResampler(resampler) => match resampler {
                Resampler::Linear(linear) => linear.next(),
                Resampler::Sinc(sinc) => sinc.next(),
            },
            SourceReader::MonoToStereo(source, mut accum) => {
                if accum.is_none() {
                    accum = source.next();
                }
                accum
            }
            SourceReader::StereoToMono(source) => {
                if let Some(left) = source.next() {
                    let right = source.next().unwrap_or_else(SampleFormat::equilibrium);
                    Some(left.add_amp(right))
                } else {
                    None
                }
            }
        }
    }
}
