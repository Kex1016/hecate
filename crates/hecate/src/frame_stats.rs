//! Per-output frame timing for `hecate msg frames`.

use std::collections::VecDeque;
use std::time::Duration;

use hecate_ipc as ipc;

/// Frames kept for the averages.
const WINDOW: usize = 240;

#[derive(Debug, Default)]
pub struct FrameStats {
    pub frames: u64,
    pub missed: u64,
    render: VecDeque<Duration>,
    scene: VecDeque<Duration>,
    draw: VecDeque<Duration>,
    queue: VecDeque<Duration>,
    /// Submit to presentation.
    latency: VecDeque<Duration>,
    /// Between presentations.
    interval: VecDeque<Duration>,
    submitted_at: Option<Duration>,
    last_presented: Option<Duration>,
}

fn push(q: &mut VecDeque<Duration>, d: Duration) {
    if q.len() == WINDOW {
        q.pop_front();
    }
    q.push_back(d);
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn avg(q: &VecDeque<Duration>) -> f64 {
    if q.is_empty() {
        return 0.0;
    }
    q.iter().map(|d| ms(*d)).sum::<f64>() / q.len() as f64
}

impl FrameStats {
    /// A frame was rendered and submitted at `at`, taking `render` to build.
    pub fn submitted(&mut self, at: Duration, render: Duration) {
        push(&mut self.render, render);
        self.submitted_at = Some(at);
    }

    /// How a frame's render time split up (see [`ipc::FrameStats`]).
    pub fn phases(&mut self, scene: Duration, draw: Duration, queue: Duration) {
        push(&mut self.scene, scene);
        push(&mut self.draw, draw);
        push(&mut self.queue, queue);
    }

    /// The submitted frame reached the screen at `at`. `refresh` is the nominal frame time.
    pub fn presented(&mut self, at: Duration, refresh: Duration) {
        self.frames += 1;
        if let Some(sub) = self.submitted_at.take() {
            push(&mut self.latency, at.saturating_sub(sub));
        }
        if let Some(last) = self.last_presented.replace(at) {
            let gap = at.saturating_sub(last);
            push(&mut self.interval, gap);
            // A gap over 1.5 frames while rendering continuously is a missed vblank. Idle gaps
            // (nothing to draw) aren't: only count up to a few frames.
            if gap > refresh.mul_f64(1.5) && gap < refresh * 4 {
                self.missed += (gap.as_secs_f64() / refresh.as_secs_f64()).round() as u64 - 1;
            }
        }
    }

    pub fn ipc(&self, output: String, refresh_hz: f64) -> ipc::FrameStats {
        let interval = avg(&self.interval);
        ipc::FrameStats {
            output,
            refresh_hz,
            frames: self.frames,
            missed: self.missed,
            avg_render_ms: avg(&self.render),
            max_render_ms: self.render.iter().map(|d| ms(*d)).fold(0.0, f64::max),
            avg_latency_ms: avg(&self.latency),
            avg_interval_ms: interval,
            fps: if interval > 0.0 { 1000.0 / interval } else { 0.0 },
            avg_scene_ms: avg(&self.scene),
            avg_draw_ms: avg(&self.draw),
            avg_queue_ms: avg(&self.queue),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_frames_latency_and_missed_vblanks() {
        let mut s = FrameStats::default();
        let frame = Duration::from_micros(16_667);
        let mut t = Duration::from_secs(1);
        for i in 0..10 {
            s.submitted(t, Duration::from_millis(2));
            // Frame 5 slips by one vblank.
            let present = t + Duration::from_millis(4) + if i == 5 { frame } else { Duration::ZERO };
            s.presented(present, frame);
            t = present - Duration::from_millis(4) + frame;
        }
        let r = s.ipc("o".into(), 60.0);
        assert_eq!(r.frames, 10);
        assert_eq!(r.missed, 1);
        assert!((r.avg_render_ms - 2.0).abs() < 1e-6);
        // Nine frames at 4 ms, the slipped one a frame later.
        let expected = (9.0 * 4.0 + 4.0 + 16.667) / 10.0;
        assert!((r.avg_latency_ms - expected).abs() < 0.01, "{}", r.avg_latency_ms);
        assert!(r.fps > 50.0 && r.fps < 61.0, "{}", r.fps);
    }

    #[test]
    fn idle_gaps_are_not_missed_frames() {
        let mut s = FrameStats::default();
        let frame = Duration::from_micros(16_667);
        s.presented(Duration::from_secs(1), frame);
        s.presented(Duration::from_secs(5), frame);
        assert_eq!(s.missed, 0);
    }
}
