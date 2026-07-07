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

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_tracer_creation() {
        let tracer = Tracer::new("test_operation");
        assert_eq!(tracer.name, "test_operation");

        // Verify that start_time is reasonable (recent)
        let now = Instant::now();
        let time_diff = now.duration_since(tracer.start_time);
        assert!(time_diff < Duration::from_millis(10)); // Should be created very recently
    }

    #[test]
    fn test_tracer_timing() {
        let start = Instant::now();
        {
            let _tracer = Tracer::new("sleep_test");
            thread::sleep(Duration::from_millis(10));
            // Tracer will drop here and print timing
        }
        let total_elapsed = start.elapsed();

        // Test should take at least 10ms due to sleep
        assert!(total_elapsed >= Duration::from_millis(10));
        assert!(total_elapsed < Duration::from_millis(100)); // Should be reasonable
    }

    #[test]
    fn test_tracer_name_storage() {
        let tracer1 = Tracer::new("operation_one");
        let tracer2 = Tracer::new("operation_two");

        assert_eq!(tracer1.name, "operation_one");
        assert_eq!(tracer2.name, "operation_two");
        assert_ne!(tracer1.name, tracer2.name);
    }

    #[test]
    fn test_tracer_elapsed_time_progression() {
        let tracer = Tracer::new("progression_test");

        let time1 = tracer.start_time.elapsed();
        thread::sleep(Duration::from_millis(5));
        let time2 = tracer.start_time.elapsed();

        // Second measurement should be larger than first
        assert!(time2 > time1);
        assert!(time2 - time1 >= Duration::from_millis(5));
    }

    #[test]
    fn test_tracer_multiple_instances() {
        let tracer1 = Tracer::new("instance_1");
        thread::sleep(Duration::from_millis(1));
        let tracer2 = Tracer::new("instance_2");

        // Different tracers should have different start times
        assert!(tracer2.start_time > tracer1.start_time);

        // First tracer should show more elapsed time
        let elapsed1 = tracer1.start_time.elapsed();
        let elapsed2 = tracer2.start_time.elapsed();
        assert!(elapsed1 > elapsed2);
    }

    #[test]
    fn test_tracer_raii_behavior() {
        // Test that tracer properly implements RAII pattern
        let outer_start = Instant::now();

        {
            let _tracer = Tracer::new("raii_test");
            // Do some work
            thread::sleep(Duration::from_millis(5));
            // Tracer should drop and print timing when scope ends
        }

        let outer_elapsed = outer_start.elapsed();
        assert!(outer_elapsed >= Duration::from_millis(5));

        // After the scope, tracer should be dropped
        // (We can't test the Drop directly, but we know it happened)
    }

    #[test]
    fn test_tracer_zero_time_edge_case() {
        let tracer = Tracer::new("zero_time_test");

        // Immediately check elapsed time
        let elapsed = tracer.start_time.elapsed();

        // Should be very small but not necessarily zero due to system timing
        assert!(elapsed < Duration::from_millis(1));
    }
}
