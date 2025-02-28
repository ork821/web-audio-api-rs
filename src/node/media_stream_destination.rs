use std::error::Error;

use crate::buffer::AudioBuffer;
use crate::context::{AudioContextRegistration, BaseAudioContext};
use crate::render::{AudioParamValues, AudioProcessor, AudioRenderQuantum};
use crate::SampleRate;

use super::{AudioNode, ChannelConfig, ChannelConfigOptions, MediaStream};

use crossbeam_channel::{self, Receiver, Sender};

/// An audio stream destination (e.g. WebRTC sink)
///
/// - MDN documentation: <https://developer.mozilla.org/en-US/docs/Web/API/MediaStreamAudioDestinationNode>
/// - specification: <https://www.w3.org/TR/webaudio/#mediastreamaudiodestinationnode>
/// - see also: [`AudioContext::create_media_stream_destination`](crate::context::AudioContext::create_media_stream_destination)
///
/// Since the w3c `MediaStream` interface is not part of this library, we cannot adhere to the
/// official specification. Instead, you can pass in any callback that handles audio buffers.
///
/// IMPORTANT: you must consume the buffers faster than the render thread produces them, or you
/// will miss frames. Consider to spin up a dedicated thread to consume the buffers and cache them.
///
/// # Usage
///
/// ```no_run
/// use web_audio_api::context::{AudioContext, BaseAudioContext};
/// use web_audio_api::node::{AudioNode, AudioScheduledSourceNode};
///
/// // Create an audio context where all audio nodes lives
/// let context = AudioContext::default();
///
/// // Create an oscillator node with sine (default) type
/// let osc = context.create_oscillator();
///
/// // Create a media destination node
/// let dest = context.create_media_stream_destination();
/// osc.connect(&dest);
/// osc.start();
///
/// // Handle recorded buffers
/// println!("samples recorded:");
/// let mut samples_recorded = 0;
/// for item in dest.stream() {
///     let buffer = item.unwrap();
///
///     // You could write the samples to a file here.
///     samples_recorded += buffer.length();
///     print!("{}\r", samples_recorded);
/// }
/// ```
///
/// # Examples
///
/// - `cargo run --release --example recorder`

pub struct MediaStreamAudioDestinationNode {
    registration: AudioContextRegistration,
    channel_config: ChannelConfig,
    receiver: Receiver<AudioBuffer>,
}

impl AudioNode for MediaStreamAudioDestinationNode {
    fn registration(&self) -> &AudioContextRegistration {
        &self.registration
    }

    fn channel_config(&self) -> &ChannelConfig {
        &self.channel_config
    }

    fn number_of_inputs(&self) -> usize {
        1
    }

    fn number_of_outputs(&self) -> usize {
        0
    }
}

impl MediaStreamAudioDestinationNode {
    /// Create a new MediaStreamAudioDestinationNode
    pub fn new<C: BaseAudioContext>(context: &C, options: ChannelConfigOptions) -> Self {
        context.base().register(move |registration| {
            let (send, recv) = crossbeam_channel::bounded(1);
            let recv_control = recv.clone();

            let node = MediaStreamAudioDestinationNode {
                registration,
                channel_config: options.into(),
                receiver: recv_control,
            };

            let render = DestinationRenderer { send, recv };

            (node, Box::new(render))
        })
    }

    /// A [`MediaStream`] iterator producing audio buffers with the same number of channels as the
    /// node itself
    ///
    /// Note that while you can call this function multiple times and poll all iterators concurrently,
    /// this could lead to unexpected behavior as the buffers will only be offered once.
    pub fn stream(&self) -> impl MediaStream {
        AudioDestinationNodeStream {
            receiver: self.receiver.clone(),
        }
    }
}

struct DestinationRenderer {
    send: Sender<AudioBuffer>,
    recv: Receiver<AudioBuffer>,
}

impl AudioProcessor for DestinationRenderer {
    fn process(
        &mut self,
        inputs: &[AudioRenderQuantum],
        _outputs: &mut [AudioRenderQuantum],
        _params: AudioParamValues,
        _timestamp: f64,
        sample_rate: SampleRate,
    ) -> bool {
        // single input, no output
        let input = &inputs[0];

        // convert AudioRenderQuantum to AudioBuffer
        let samples: Vec<_> = input
            .channels()
            .iter()
            .map(|c| c.as_slice().to_vec())
            .collect();
        let buffer = AudioBuffer::from(samples, sample_rate);

        // clear previous entry if it was not consumed
        let _ = self.recv.try_recv();

        // ship out AudioBuffer
        let _ = self.send.send(buffer);

        false
    }
}

// no need for public documentation because the concrete type is never returned (an impl
// MediaStream is returned instead)
#[doc(hidden)]
pub struct AudioDestinationNodeStream {
    receiver: Receiver<AudioBuffer>,
}

impl Iterator for AudioDestinationNodeStream {
    type Item = Result<AudioBuffer, Box<dyn Error + Send + Sync>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.receiver.recv() {
            Ok(buf) => Some(Ok(buf)),
            Err(e) => Some(Err(Box::new(e))),
        }
    }
}
