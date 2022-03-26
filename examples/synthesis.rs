use rand::Rng;
use std::fs::File;

use web_audio_api::context::{BaseAudioContext, OfflineAudioContext};
use web_audio_api::node::AudioNode;
use web_audio_api::SampleRate;

fn main() {
    const DURATION: usize = 120;
    let sample_rate = SampleRate(48000);

    let adjusted_duration = DURATION as f64 / 16.;
    let length = (adjusted_duration * sample_rate.0 as f64) as usize;
    let mut context = OfflineAudioContext::new(1, length, sample_rate);

    let buffer = {
        let context = OfflineAudioContext::new(1, 1, sample_rate);
        let file = File::open("samples/think-mono-48000.wav").unwrap();
        context.decode_audio_data_sync(file).unwrap()
    };

    let mut offset = 0.;
    let mut rng = rand::thread_rng();

    // @todo - make a PR
    // - problem w/ env.gain().set_value_at_time(0., offset);
    // - variables are badly named, but just follow the source here

    // this 1500 sources...
    while offset < adjusted_duration {
        let env = context.create_gain();
        env.connect(&context.destination());

        let src = context.create_buffer_source();
        src.connect(&env);
        src.set_buffer(buffer.clone());

        let rand_start = rng.gen_range(0..1000u32) as f64 / 1000. * 0.5;
        let rand_duration = rng.gen_range(0..1000u32) as f64 / 1000. * 0.999;
        let start = offset * rand_start;
        let end = start + 0.005 * rand_duration;
        let start_release = (offset + end - start).max(0.);

        env.gain().set_value_at_time(0., offset);
        env.gain().linear_ramp_to_value_at_time(0.5, offset + 0.005);
        env.gain().set_value_at_time(0.5, start_release);
        env.gain()
            .linear_ramp_to_value_at_time(0., start_release + 0.05);

        src.start_at_with_offset_and_duration(offset, start, end);

        offset += 0.005;
    }

    let buffer = context.start_rendering_sync();
    assert!(buffer.length() > 1000);
}
