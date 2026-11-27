//! A sound as it is to be heard: from where to where in it, coming up and
//! dying away, and to the left or the right.

use std::time::Duration;

use rodio::{ChannelCount, SampleRate, Source};

/// A sound shaped as [`super::Options`] say, after it was cut to its
/// part: how many frames it lasts, if known, to die away before its end.
pub struct Shape<S> {
    input: S,
    channels: u16,
    /// Frames heard so far, and how many there are, if known.
    frame: u64,
    sample_in_frame: u16,
    frames: Option<u64>,
    fade_in: u64,
    fade_out: u64,
    /// The gain of each side, for a sound to one side.
    sides: [f32; 2],
    /// A sound of one channel put to one side is made of two: the right
    /// side of the frame being heard, when the left was.
    widened: bool,
    right: Option<f32>,
}

impl<S: Source> Shape<S> {
    /// `pan` from -1, all left, to 1, all right; 0 in the middle.
    pub fn new(input: S, length: Option<Duration>, fades: (Duration, Duration), pan: f32) -> Self {
        let rate = f64::from(input.sample_rate().get());
        let frames = |duration: Duration| (duration.as_secs_f64() * rate) as u64;
        let channels = input.channels().get();
        Self {
            channels,
            frame: 0,
            sample_in_frame: 0,
            frames: length.map(frames),
            fade_in: frames(fades.0),
            fade_out: frames(fades.1),
            sides: sides(pan),
            widened: channels == 1 && pan != 0.0,
            right: None,
            input,
        }
    }

    /// How loud the frame being heard is, coming up and dying away.
    fn gain(&self) -> f32 {
        let mut gain = 1.0;
        if self.fade_in > 0 && self.frame < self.fade_in {
            gain *= self.frame as f32 / self.fade_in as f32;
        }
        if let Some(frames) = self.frames.filter(|_| self.fade_out > 0) {
            let left = frames.saturating_sub(self.frame);
            if left < self.fade_out {
                gain *= left as f32 / self.fade_out as f32;
            }
        }
        gain
    }
}

/// The gain of the left and the right, keeping the sound as loud wherever
/// it is put: in the middle, both as they were.
fn sides(pan: f32) -> [f32; 2] {
    let pan = pan.clamp(-1.0, 1.0);
    if pan == 0.0 {
        return [1.0, 1.0];
    }
    let angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;
    let root_two = std::f32::consts::SQRT_2;
    [angle.cos() * root_two, angle.sin() * root_two].map(|side| side.min(1.0))
}

impl<S: Source> Iterator for Shape<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        let sample = self.input.next()?;
        if self.widened {
            let sample = sample * self.gain();
            self.frame += 1;
            self.right = Some(sample * self.sides[1]);
            return Some(sample * self.sides[0]);
        }
        let mut sample = sample * self.gain();
        // Two channels: each side as the pan says.
        if self.channels == 2 {
            sample *= self.sides[usize::from(self.sample_in_frame)];
        }
        self.sample_in_frame += 1;
        if self.sample_in_frame >= self.channels {
            self.sample_in_frame = 0;
            self.frame += 1;
        }
        Some(sample)
    }
}

impl<S: Source> Source for Shape<S> {
    fn current_span_len(&self) -> Option<usize> {
        let span = self.input.current_span_len();
        match self.widened {
            true => span.map(|span| span * 2),
            false => span,
        }
    }

    fn channels(&self) -> ChannelCount {
        match self.widened {
            true => ChannelCount::new(2).unwrap_or(self.input.channels()),
            false => self.input.channels(),
        }
    }

    fn sample_rate(&self) -> SampleRate {
        self.input.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }
}
