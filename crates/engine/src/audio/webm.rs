//! Streams `WebM` audio through Kira without requiring a per-track sample count.
//! Symphonia's Matroska reader exposes the duration on the container instead.

use kira::Frame;
use kira::sound::FromFileError;
use num_traits::ToPrimitive;
use symphonia::core::codecs::{
    CodecParameters,
    audio::{AudioDecoder, AudioDecoderOptions},
};
use symphonia::core::formats::{FormatReader, TrackType};

use crate::files::GameFiles;
use crate::media;

pub(super) struct Decoder {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    files: GameFiles,
    path: String,
    sample_rate: u32,
    num_frames: usize,
    position: usize,
}

impl Decoder {
    pub(super) fn open(files: &GameFiles, path: &str) -> Result<Self, FromFileError> {
        let reader = media::open(files, path)?;
        let track = reader
            .default_track(TrackType::Audio)
            .ok_or(FromFileError::NoDefaultTrack)?;
        let Some(CodecParameters::Audio(params)) = &track.codec_params else {
            return Err(FromFileError::NoDefaultTrack);
        };
        let sample_rate = params.sample_rate.ok_or(FromFileError::UnknownSampleRate)?;
        let duration = media::duration(reader.as_ref()).ok_or(FromFileError::UnknownDuration)?;
        let num_frames = (duration * f64::from(sample_rate))
            .ceil()
            .to_usize()
            .ok_or(FromFileError::UnknownDuration)?;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())?;
        let track_id = track.id;
        Ok(Self {
            reader,
            decoder,
            track_id,
            files: files.clone(),
            path: path.to_owned(),
            sample_rate,
            num_frames,
            position: 0,
        })
    }
}

impl kira::sound::streaming::Decoder for Decoder {
    type Error = FromFileError;

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn num_frames(&self) -> usize {
        self.num_frames
    }

    fn decode(&mut self) -> Result<Vec<Frame>, Self::Error> {
        loop {
            let Some(packet) = self.reader.next_packet()? else {
                // The container duration may include video after the audio ends.
                // Kira expects samples through num_frames, including that silence.
                let count = self.num_frames.saturating_sub(self.position).min(4096);
                self.position += count;
                return Ok(vec![Frame::ZERO; count]);
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let buffer = self.decoder.decode(&packet)?;
            if buffer.frames() == 0 {
                continue;
            }
            let mut samples = Vec::<f32>::new();
            buffer.copy_to_vec_interleaved(&mut samples);
            let frames: Vec<_> = match buffer.num_planes() {
                1 => samples.into_iter().map(Frame::from_mono).collect(),
                2 => samples
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| Frame::new(pair[0], pair[1]))
                    .collect(),
                _ => return Err(FromFileError::UnsupportedChannelConfiguration),
            };
            self.position += frames.len();
            return Ok(frames);
        }
    }

    fn seek(&mut self, _index: usize) -> Result<usize, Self::Error> {
        // Reopen at the beginning for loops and preserve Vorbis overlap history
        // when seeking. Kira skips samples to the requested index on its decoder
        // thread. Symphonia's Matroska reader cannot seek back after reaching EOF.
        self.reader = media::open(&self.files, &self.path)?;
        self.decoder.reset();
        self.position = 0;
        Ok(0)
    }
}
