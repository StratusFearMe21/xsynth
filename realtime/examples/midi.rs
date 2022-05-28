use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use core::{
    channel::{ChannelEvent, ControlEvent},
    soundfont::{SoundfontBase, SquareSoundfont},
};

use midi_toolkit::{
    events::{Event, MIDIEvent},
    io::MIDIFile,
    pipe,
    sequence::{
        event::{cancel_tempo_events, flatten_batches_to_events, scale_event_time},
        unwrap_items, TimeCaster,
    },
};
use xsynth_realtime::{RealtimeSynth, SynthEvent};

fn main() {
    let synth = RealtimeSynth::open_with_all_defaults();
    let mut sender = synth.get_senders();

    let params = synth.stream_params();

    let soundfonts: Vec<Arc<dyn SoundfontBase>> = vec![Arc::new(SquareSoundfont::new(
        params.sample_rate,
        params.channels,
    ))];

    sender.send_event(SynthEvent::AllChannels(ChannelEvent::SetSoundfonts(
        soundfonts,
    )));

    let stats = synth.get_stats();
    thread::spawn(move || loop {
        println!(
            "Voice Count: {}  \tBuffer: {}\tRender time: {}",
            stats.voice_count(),
            stats.buffer().last_samples_after_read(),
            stats.buffer().average_renderer_load()
        );
        thread::sleep(Duration::from_millis(10));
    });

    let midi = MIDIFile::open("F:/Fast MIDIs/The Nuker 4 F2.mid", None).unwrap();

    let ppq = midi.ppq();
    let merged = pipe!(
        midi.iter_all_events_merged_batches()
        // |>threaded_buffer(200000)
        |>flatten_batches_to_events()
        |>TimeCaster::<f64>::cast_event_delta()
        |>cancel_tempo_events(250000)
        |>scale_event_time(1.0 / ppq as f64)
        |>unwrap_items()
    );

    // let (tx, rx) = crossbeam_channel::bounded(16777216);

    // thread::spawn(move || {
    //     for e in merged {
    //         tx.send(e).unwrap();
    //     }
    // });

    let now = Instant::now() - Duration::from_secs_f64(0.0);
    let mut time = 0.0;
    for e in merged {
        if e.delta() != 0.0 {
            time += e.delta();
            let diff = time - now.elapsed().as_secs_f64();
            if diff > 0.0 {
                spin_sleep::sleep(Duration::from_secs_f64(diff));
            }
        }

        match e {
            Event::NoteOn(e) => {
                sender.send_event(SynthEvent::Channel(
                    e.channel as u32,
                    ChannelEvent::NoteOn {
                        key: e.key,
                        vel: e.velocity,
                    },
                ));
            }
            Event::NoteOff(e) => {
                sender.send_event(SynthEvent::Channel(
                    e.channel as u32,
                    ChannelEvent::NoteOff { key: e.key },
                ));
            }
            Event::ControlChange(e) => {
                sender.send_event(SynthEvent::Channel(
                    e.channel as u32,
                    ChannelEvent::Control(ControlEvent::Raw(e.controller, e.value)),
                ));
            }
            Event::PitchWheelChange(e) => {
                sender.send_event(SynthEvent::Channel(
                    e.channel as u32,
                    ChannelEvent::Control(ControlEvent::PitchBendValue(e.pitch as f32 / 8192.0)),
                ));
            }
            _ => {}
        }
    }

    std::thread::sleep(Duration::from_secs(10000));
}
