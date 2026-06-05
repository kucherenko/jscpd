use std::time::{Duration, Instant};

pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn start() -> Self {
        Timer {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_measures_elapsed_time() {
        let timer = Timer::start();

        // Simulate some work
        std::thread::sleep(Duration::from_millis(10));

        let elapsed = timer.elapsed();

        // Should be at least 10ms
        assert!(elapsed.as_millis() >= 10);
        // Should be less than 1 second (sanity check)
        assert!(elapsed.as_secs() < 1);
    }

    #[test]
    fn timer_overhead_is_minimal() {
        let timer = Timer::start();
        let elapsed = timer.elapsed();

        // Overhead should be < 5ms
        assert!(elapsed.as_millis() < 5);
    }

    #[test]
    fn timer_can_be_queried_multiple_times() {
        let timer = Timer::start();

        std::thread::sleep(Duration::from_millis(5));
        let elapsed1 = timer.elapsed();

        std::thread::sleep(Duration::from_millis(5));
        let elapsed2 = timer.elapsed();

        // Second measurement should be greater
        assert!(elapsed2 > elapsed1);
        // Both should be reasonable
        assert!(elapsed1.as_millis() >= 5);
        assert!(elapsed2.as_millis() >= 10);
    }
}
