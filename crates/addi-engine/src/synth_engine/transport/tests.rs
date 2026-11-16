use super::*;

/// 120 BPM at the start of the block. Used by engine tests that do not model a host.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DefaultTransport;

impl HostTransport for DefaultTransport {
    fn tempo(&self) -> f64 {
        120.0
    }

    fn pos_seconds(&self) -> f64 {
        0.0
    }

    fn pos_beats(&self) -> f64 {
        0.0
    }

    fn origin(&self) -> isize {
        0
    }
}

struct FixedTransport {
    origin: isize,
}

impl HostTransport for FixedTransport {
    fn tempo(&self) -> f64 {
        120.0
    }

    fn pos_seconds(&self) -> f64 {
        10.0
    }

    fn pos_beats(&self) -> f64 {
        20.0
    }

    fn origin(&self) -> isize {
        self.origin
    }
}

fn transport_at(origin: isize) -> FixedTransport {
    FixedTransport { origin }
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

#[test]
fn timeline_measures_from_transport_offset() {
    let sample_rate = 48_000.0;
    let timeline = HostTimeline::from_transport(sample_rate, &transport_at(28), 1);

    let seconds = 28.0 / sample_rate;
    assert_close(timeline.seconds_at(0), 10.0 + seconds);
    assert_close(timeline.beats_at(0), 20.0 + seconds * 2.0);
}

#[test]
fn timeline_reaches_back_when_block_starts_before_transport() {
    let sample_rate = 48_000.0;
    let timeline = HostTimeline::from_transport(sample_rate, &transport_at(-36), 1);

    let seconds = 36.0 / sample_rate;
    assert_close(timeline.seconds_at(0), 10.0 - seconds);
    assert_close(timeline.beats_at(0), 20.0 - seconds * 2.0);
}

#[test]
fn oversampling_keeps_host_sample_duration() {
    let host_rate = 48_000.0;
    let timeline = HostTimeline::from_transport(host_rate * 2.0, &transport_at(28), 2);

    assert_close(timeline.seconds_at(0), 10.0 + 28.0 / host_rate);
}
