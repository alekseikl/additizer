use super::*;

#[test]
fn published_length_follows_the_source() {
    let (mut audio, mut ui) = make_link_pair();
    let long = vec![ComplexSample::from_polar(0.25, 0.1); DISPLAY_SPECTRUM_SIZE + 8];

    audio.update_spectrum(&long);
    assert_eq!(ui.get_spectrum().len(), DISPLAY_SPECTRUM_SIZE);
    assert_eq!(ui.get_spectrum()[3], long[3]);

    let short = vec![ComplexSample::from_polar(0.5, -0.2); 6];
    audio.update_spectrum(&short);
    assert_eq!(ui.get_spectrum(), short.as_slice());
    assert!(audio.spectrum.input_buffer().capacity() >= DISPLAY_SPECTRUM_SIZE);
}
