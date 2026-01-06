pub mod instant;
pub use instant::Instant;

#[derive(Clone, Debug)]
pub struct Timer {
    start: Instant,
}

impl Default for Timer {
    fn default() -> Self {
        Timer::new()
    }
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> f32 {
        self.start.elapsed().as_secs_f32()
    }

    pub fn reset(&mut self) {
        self.start = Instant::now();
    }
}

#[derive(Clone, Debug, Default)]
pub struct FpsCounter {
    timer: Timer,
    elapsed_time: f32,
    elapsed_frames: u32,
    fps: u32,
}

impl FpsCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self) -> bool {
        let delta_time = self.timer.elapsed();
        self.timer.reset();

        self.elapsed_time += delta_time;
        self.elapsed_frames += 1;

        if self.elapsed_time >= 1.0 {
            self.fps = self.elapsed_frames;

            self.elapsed_time -= 1.0;
            self.elapsed_frames = 0;

            true
        } else {
            false
        }
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn ms(&self) -> f32 {
        1000.0 / self.fps as f32
    }
}
