use additizer::Additizer;
use nice_plug::prelude::*;

fn first_midi_input_name() -> Option<String> {
    let midi = midir::MidiInput::new("additizer").ok()?;
    let ports = midi.ports();
    ports.first().and_then(|port| midi.port_name(port).ok())
}

fn main() {
    let mut args: Vec<String> = std::env::args().collect();

    if !args.iter().any(|arg| arg == "--midi-input")
        && let Some(name) = first_midi_input_name()
    {
        args.push("--midi-input".to_string());
        args.push(name);
    }

    nice_export_standalone_with_args::<Additizer, _>(args);
}
