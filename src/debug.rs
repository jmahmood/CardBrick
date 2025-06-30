use std::time::Instant;

/// A simple RAII timer for performance tracing.
/// When created, it notes the start time.
/// When it goes out of scope (at the end of a block), it automatically
/// prints the time elapsed since its creation.
pub struct Tracer {
    name: &'static str,
    start_time: Instant,
}

impl Tracer {
    pub fn new(name: &'static str) -> Self {
        Tracer {
            name,
            start_time: Instant::now(),
        }
    }
}

impl Drop for Tracer {
    fn drop(&mut self) {
        let elapsed = self.start_time.elapsed();
        // Print in a human-readable format (e.g., milliseconds or microseconds)
        println!("[Trace] {}: {:.2?}", self.name, elapsed);
    }
}