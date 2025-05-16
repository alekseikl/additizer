pub fn normalize_phase(phase: f32) -> f32 {
    if phase < 0.0 {
        1.0 + phase.fract()
    } else {
        phase.fract()
    }
}
