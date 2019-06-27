use cpal::{
    ChannelCount, Device, EventLoop, Format, SampleRate, StreamData, StreamId, SupportedFormat,
    UnknownTypeOutputBuffer,
};
use sample::{
    conv::ToSample,
    frame::Frame,
    interpolate, ring_buffer,
    signal::{self, Signal},
    Sample,
};

use std::{iter, num::NonZeroU32, sync::mpsc, thread};

use super::{
    mix::Mixer, source, CanonicalFrame, CanonicalSample, CanonicalSignal,
    HIGH_QUALITY_INTERPOLATION,
};

use crate::assets::Asset;

pub struct SinkSampleRate(Option<NonZeroU32>);

impl SinkSampleRate {
    pub fn resample_from<S, F>(
        &self,
        in_rate: u32,
        mut signal: S,
        //) -> Box<dyn Signal<Frame = F> + Send + Sync>
    ) -> Box<dyn CanonicalSignal + Send + Sync>
    where
        S: Signal<Frame = F> + Send + Sync + 'static,
        F: Frame<Sample = CanonicalSample> + Send + Sync + 'static,
        F::Channels: Send + Sync + 'static,
    {
        match self.0 {
            None => Box::new(signal),
            Some(out_rate) if in_rate == out_rate.get() => Box::new(signal),
            Some(out_rate) => {
                if HIGH_QUALITY_INTERPOLATION {
                    let buffer = ring_buffer::Fixed::from([F::equilibrium(); 100]);
                    let sinc = interpolate::Sinc::new(buffer);
                    Box::new(signal.from_hz_to_hz(sinc, in_rate.into(), out_rate.get().into()))
                } else {
                    let linear = interpolate::Linear::from_source(&mut signal);
                    Box::new(signal.from_hz_to_hz(linear, in_rate.into(), out_rate.get().into()))
                }
            }
        }
    }
}

pub trait Sink {
    fn load(&self, asset: &'static Asset) -> Box<dyn CanonicalSignal + Send + Sync>;

    fn play(&self, name: Option<&'static str>, signal: Box<dyn CanonicalSignal + Send + Sync>);
    fn play_singleton(&self, name: &'static str, signal: Box<dyn CanonicalSignal + Send + Sync>);

    fn channels(&self) -> Option<ChannelCount>;
    fn sample_rate(&self) -> SinkSampleRate;
}

struct DummySink;

impl Sink for DummySink {
    fn load(&self, _asset: &'static Asset) -> Box<dyn CanonicalSignal + Send + Sync> {
        Box::new(signal::from_iter(iter::empty::<CanonicalFrame>()))
    }

    fn play(&self, _name: Option<&'static str>, _signal: Box<dyn CanonicalSignal + Send + Sync>) {}
    fn play_singleton(&self, _name: &'static str, _signal: Box<dyn CanonicalSignal + Send + Sync>) {
    }

    fn channels(&self) -> Option<ChannelCount> {
        None
    }
    fn sample_rate(&self) -> SinkSampleRate {
        SinkSampleRate(None)
    }
}

enum AudioCommand {
    Play(Option<&'static str>, Box<dyn CanonicalSignal + Send + Sync>),
    PlaySingleton(&'static str, Box<dyn CanonicalSignal + Send + Sync>),
}

pub struct RealSink {
    sender: mpsc::Sender<AudioCommand>,
    format: Format,
}

impl Sink for RealSink {
    fn load(&self, asset: &'static Asset) -> Box<dyn CanonicalSignal + Send + Sync> {
        source::new(&asset, &self)
    }

    fn play(&self, name: Option<&'static str>, signal: Box<dyn CanonicalSignal + Send + Sync>) {
        self.sender.send(AudioCommand::Play(name, signal));
    }

    fn play_singleton(&self, name: &'static str, signal: Box<dyn CanonicalSignal + Send + Sync>) {
        self.sender.send(AudioCommand::PlaySingleton(name, signal));
    }

    fn channels(&self) -> Option<ChannelCount> {
        Some(self.format.channels)
    }

    fn sample_rate(&self) -> SinkSampleRate {
        SinkSampleRate(Some(NonZeroU32::new(self.format.sample_rate.0).unwrap()))
    }
}

struct AudioThread {
    receiver: mpsc::Receiver<AudioCommand>,
    mixer: Mixer<CanonicalFrame>,
    format: Format,
}

impl AudioThread {
    pub fn start() -> Box<dyn Sink> {
        match Self::spawn() {
            Ok(real_sink) => Box::new(real_sink),
            Err(_) => Box::new(DummySink {}),
        }
    }

    fn spawn() -> Result<RealSink, ()> {
        let device = Self::get_output_device()?;
        let format = Self::get_output_format(&device)?;

        let event_loop = EventLoop::new();
        event_loop.play_stream(
            event_loop
                .build_output_stream(&device, &format)
                .or(Err(()))?,
        );

        let (sender, receiver) = mpsc::channel();
        let thread_format = format.clone();
        thread::spawn(move || {
            let mut t = Self {
                receiver,
                mixer: Mixer::new(),
                format: thread_format,
            };

            event_loop.run(move |id, data| t.callback(id, data));
        });

        Ok(RealSink { sender, format })
    }

    pub fn get_output_device() -> Result<Device, ()> {
        // TODO: sound device selection menu
        // see issue #2
        cpal::default_output_device().ok_or(())
    }

    pub fn get_output_format(device: &Device) -> Result<Format, ()> {
        const HZ_44100: Option<SampleRate> = Some(SampleRate(44100));

        match device
            .supported_output_formats()
            .ok()
            .and_then(|s| s.max_by(SupportedFormat::cmp_default_heuristics))
        {
            Some(SupportedFormat {
                channels,
                min_sample_rate,
                max_sample_rate,
                data_type,
            }) => Some(Format {
                channels,
                sample_rate: HZ_44100
                    .filter(|r| (min_sample_rate..=max_sample_rate).contains(r))
                    .unwrap_or(max_sample_rate),
                data_type,
            }),
            None => device.default_output_format().ok(),
        }
        .filter(|f| f.channels <= 2)
        .ok_or(())
    }

    fn callback(&mut self, _id: StreamId, data: StreamData) {
        self.process_commands();

        match data {
            StreamData::Output {
                buffer: UnknownTypeOutputBuffer::U16(mut buffer),
            } => self.fill_stream_buffer(&mut buffer),
            StreamData::Output {
                buffer: UnknownTypeOutputBuffer::I16(mut buffer),
            } => self.fill_stream_buffer(&mut buffer),
            StreamData::Output {
                buffer: UnknownTypeOutputBuffer::F32(mut buffer),
            } => self.fill_stream_buffer(&mut buffer),
            _ => (),
        }
    }

    fn process_commands(&mut self) {
        for command in self.receiver.try_iter() {
            match command {
                AudioCommand::Play(name, signal) => {
                    self.mixer.add(name, signal.into_interleaved_signal());
                }
                AudioCommand::PlaySingleton(name, signal) => {
                    self.mixer.remove(name);
                    self.mixer.add(Some(name), signal.into_interleaved_signal());
                }
            }
        }
    }

    fn fill_stream_buffer<O>(&mut self, buffer: &mut [O])
    where
        CanonicalSample: Sample + ToSample<O>,
    {
        // NOTE: it would not be correct to directly copy interleaved samples
        // instead of doing it on a frame-by-frame basis were it not for the
        // implementation of source::new, which dynamically ensures the frame
        // width is the same as the sink's (by doubling mono or mixing stereo).
        for sample in buffer {
            *sample = self.mixer.next()[0].to_sample();
        }
    }
}

// TODO: document why Mixer is always mono
