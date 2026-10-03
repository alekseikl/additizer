use super::*;

const SAMPLE_RATE: Sample = 48_000.0;

#[test]
fn silence_stays_at_zero() {
    let mut ballistics = LevelBallistics::default();

    assert_eq!(ballistics.process(&[0.0; 64], SAMPLE_RATE), 0.0);
}

#[test]
fn empty_buffer_keeps_level() {
    let mut ballistics = LevelBallistics::default();

    let level = ballistics.process(&[1.0; 64], SAMPLE_RATE);

    assert!(level > 0.0);
    assert_eq!(ballistics.process(&[], SAMPLE_RATE), level);
}

#[test]
fn attack_rises_faster_than_release_falls() {
    let short_block = [1.0; (SAMPLE_RATE * from_ms(5.0)) as usize];
    let silence = [0.0; (SAMPLE_RATE * from_ms(5.0)) as usize];

    let attack_level = LevelBallistics::default().process(&short_block, SAMPLE_RATE);

    let mut release = LevelBallistics { level: 1.0 };
    let release_level = release.process(&silence, SAMPLE_RATE);

    assert!(attack_level > 1.0 - release_level);
}

#[test]
fn stereo_processes_channels_independently() {
    let mut ballistics = StereoLevelBallistics::default();

    let levels = ballistics.process([&[1.0; 32], &[0.0; 32]], SAMPLE_RATE);

    assert!(levels[0] > 0.0);
    assert_eq!(levels[1], 0.0);
}
