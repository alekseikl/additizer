use addi_dsp::segments::Segments;

use super::LfoWidget;

#[test]
fn phase_shift_rotates_the_rendered_cycle() {
    let segments = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut plain = LfoWidget::default();
    let mut shifted = LfoWidget::default();
    let width = 100.0;
    let len = width as usize;
    let quarter = len / 4;

    plain.build_waveform(&segments, 0.0, width);
    shifted.build_waveform(&segments, 0.25, width);

    assert_eq!(shifted.waveform.len(), len);
    assert_eq!(shifted.waveform[0], plain.waveform[quarter]);
    assert_eq!(shifted.waveform[len - quarter], plain.waveform[0]);

    shifted.build_waveform(&segments, -0.25, width);

    assert_eq!(shifted.waveform[0], plain.waveform[len - quarter]);
}

#[test]
fn playhead_samples_the_rendered_waveform() {
    let segments = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    let mut plain = LfoWidget::default();
    let mut shifted = LfoWidget::default();
    let width = 100.0;

    plain.build_waveform(&segments, 0.0, width);
    shifted.build_waveform(&segments, 0.25, width);

    assert_eq!(plain.waveform_at(0.0), plain.waveform[0]);
    assert_eq!(plain.waveform_at(0.25), plain.waveform[25]);
    assert_eq!(plain.waveform_at(1.0), plain.waveform[0]);
    assert!(plain.waveform_at(0.252) > plain.waveform[25]);
    assert!(plain.waveform_at(0.252) < plain.waveform[26]);
    assert_eq!(shifted.waveform_at(0.0), plain.waveform[25]);
    assert_eq!(LfoWidget::default().waveform_at(0.5), 0.0);
}
