// src/scenes/studying/haptics.rs
// Haptic feedback functionality for goal achievement

/// Provides short rumble feedback when goal is achieved
/// Currently a stub - device-specific implementation needed for TrimUI Brick/RG35XX
pub fn short_rumble() {
    // TODO: Implement device-specific rumble
    // For TrimUI Brick: GPIO-based rumble motor control
    // For RG35XX Plus: Vibration motor via device driver
    // For desktop testing: Console log or audio cue

    #[cfg(test)]
    {
        println!("🎮 RUMBLE: Short haptic feedback triggered for goal achievement");
    }

    #[cfg(not(test))]
    {
        // Desktop/testing fallback - could use audio beep instead
        println!("🎮 Goal achieved! (Haptic feedback would trigger on device)");
    }
}

/// Provides longer rumble feedback for special achievements
pub fn long_rumble() {
    #[cfg(test)]
    {
        println!("🎮 RUMBLE: Long haptic feedback triggered");
    }

    #[cfg(not(test))]
    {
        println!("🎮 Special achievement! (Long haptic feedback would trigger on device)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_rumble() {
        // Should not panic and should provide some feedback
        short_rumble();
    }

    #[test]
    fn test_long_rumble() {
        // Should not panic and should provide some feedback
        long_rumble();
    }
}
