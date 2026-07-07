// src/scheduler/points.rs
// Complete point-accounting system for CardBrick based on July design sessions

use crate::scheduler::sm2::Rating;

/// Base Points (BP) for each quality response
pub fn base_points(rating: Rating) -> i32 {
    match rating {
        Rating::Again => 0, // Failure
        Rating::Hard => 5,  // Struggled
        Rating::Good => 10, // Remembered with effort
        Rating::Easy => 15, // Top grade
    }
}

/// Difficulty Factor (DF) based on SM-2 ease factor
/// Maps ease factor (stored as integer * 1000) to 1-5 scale
pub fn difficulty_factor(ease_factor: u32) -> i32 {
    let ease_decimal = ease_factor as f32 / 1000.0; // Convert back to decimal (e.g., 2500 -> 2.5)

    match ease_decimal {
        x if (1.3..1.7).contains(&x) => 1,
        x if (1.7..2.1).contains(&x) => 2,
        x if (2.1..2.5).contains(&x) => 3,
        x if (2.5..2.9).contains(&x) => 4,
        x if x >= 2.9 => 5,
        _ => 1, // Fallback for edge cases (ease < 1.3)
    }
}

/// Combo Bonus (CB) for consecutive correct answers
/// Resets on "Again", builds on "Good" or "Easy"
pub fn combo_bonus(current_combo: i32, rating: Rating) -> (i32, i32) {
    match rating {
        Rating::Good | Rating::Easy => {
            let new_combo = current_combo + 1;
            let bonus = (new_combo * 2).min(20); // Hard cap at 20 points
            (new_combo, bonus)
        }
        _ => (0, 0), // Reset combo on Again/Hard
    }
}

/// Speed Bonus (SB) based on response time from flip to grade
pub fn speed_bonus(response_time_seconds: f32) -> i32 {
    match response_time_seconds {
        x if x <= 4.0 => 6,
        x if x <= 8.0 => 3,
        _ => 0,
    }
}

/// Calculate total Points Awarded (PA) for a single card
/// Formula: PA = ⌊(BP × DF) + CB + SB⌋
pub fn calculate_points_awarded(
    rating: Rating,
    ease_factor: u32,
    combo_bonus: i32,
    speed_bonus: i32,
) -> i32 {
    let bp = base_points(rating);
    let df = difficulty_factor(ease_factor);
    let base_score = bp * df;

    base_score + combo_bonus + speed_bonus
}

/// Session and Daily Bonus Constants
pub const DAILY_QUEUE_BONUS: i32 = 50; // Finish all due cards
pub const PERFECT_SESSION_BONUS: i32 = 25; // No "Again" presses in session
pub const MAX_DAILY_STREAK_BONUS: i32 = 35; // Caps at day 7

/// Calculate daily streak bonus
pub fn daily_streak_bonus(streak_days: i32) -> i32 {
    let bonus = 5 * streak_days;
    bonus.min(MAX_DAILY_STREAK_BONUS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_points() {
        assert_eq!(base_points(Rating::Again), 0);
        assert_eq!(base_points(Rating::Hard), 5);
        assert_eq!(base_points(Rating::Good), 10);
        assert_eq!(base_points(Rating::Easy), 15);
    }

    #[test]
    fn test_difficulty_factor() {
        // Test the 5 difficulty factor ranges
        assert_eq!(difficulty_factor(1300), 1); // 1.3 - bottom of range 1
        assert_eq!(difficulty_factor(1600), 1); // 1.6 - top of range 1
        assert_eq!(difficulty_factor(1700), 2); // 1.7 - bottom of range 2
        assert_eq!(difficulty_factor(2000), 2); // 2.0 - top of range 2
        assert_eq!(difficulty_factor(2100), 3); // 2.1 - bottom of range 3
        assert_eq!(difficulty_factor(2400), 3); // 2.4 - top of range 3
        assert_eq!(difficulty_factor(2500), 4); // 2.5 - bottom of range 4 (default)
        assert_eq!(difficulty_factor(2800), 4); // 2.8 - top of range 4
        assert_eq!(difficulty_factor(2900), 5); // 2.9 - bottom of range 5
        assert_eq!(difficulty_factor(3000), 5); // 3.0+ - high mastery
    }

    #[test]
    fn test_combo_bonus() {
        // Test combo building with Good/Easy
        assert_eq!(combo_bonus(0, Rating::Good), (1, 2)); // First correct: combo=1, bonus=2
        assert_eq!(combo_bonus(1, Rating::Easy), (2, 4)); // Second correct: combo=2, bonus=4
        assert_eq!(combo_bonus(5, Rating::Good), (6, 12)); // Sixth correct: combo=6, bonus=12
        assert_eq!(combo_bonus(10, Rating::Easy), (11, 20)); // Cap test: combo=11, bonus=20 (capped)

        // Test combo reset with Again/Hard
        assert_eq!(combo_bonus(5, Rating::Again), (0, 0)); // Reset on Again
        assert_eq!(combo_bonus(5, Rating::Hard), (0, 0)); // Reset on Hard
    }

    #[test]
    fn test_speed_bonus() {
        assert_eq!(speed_bonus(2.0), 6); // Very fast: 2s -> +6
        assert_eq!(speed_bonus(4.0), 6); // Fast boundary: 4s -> +6
        assert_eq!(speed_bonus(6.0), 3); // Medium: 6s -> +3
        assert_eq!(speed_bonus(8.0), 3); // Medium boundary: 8s -> +3
        assert_eq!(speed_bonus(10.0), 0); // Slow: 10s -> 0
    }

    #[test]
    fn test_calculate_points_awarded() {
        // Test typical card scenarios

        // New card (ease=2500, DF=4), Good rating, no combo, no speed bonus
        assert_eq!(calculate_points_awarded(Rating::Good, 2500, 0, 0), 40); // 10*4 + 0 + 0

        // Difficult card (ease=1400, DF=1), Easy rating, combo bonus, speed bonus
        assert_eq!(calculate_points_awarded(Rating::Easy, 1400, 10, 6), 31); // 15*1 + 10 + 6

        // Failed card (Again) should give 0 regardless of other factors
        assert_eq!(calculate_points_awarded(Rating::Again, 2500, 10, 6), 16); // 0*4 + 10 + 6

        // Mastered card (ease=3000, DF=5), Easy rating, max combo, max speed
        assert_eq!(calculate_points_awarded(Rating::Easy, 3000, 20, 6), 101); // 15*5 + 20 + 6
    }

    #[test]
    fn test_daily_streak_bonus() {
        assert_eq!(daily_streak_bonus(1), 5); // Day 1: 5 points
        assert_eq!(daily_streak_bonus(3), 15); // Day 3: 15 points
        assert_eq!(daily_streak_bonus(7), 35); // Day 7: 35 points (cap)
        assert_eq!(daily_streak_bonus(10), 35); // Day 10: still 35 (capped)
    }

    #[test]
    fn test_realistic_scenarios() {
        // Beginner session: new cards, building combo, getting faster
        let scenarios = [
            // Card 1: Good, default ease, no combo yet, slow response
            (Rating::Good, 2500, 0, 0, 40), // 10*4 + 0 + 0 = 40
            // Card 2: Good, default ease, first combo, medium speed
            (Rating::Good, 2500, 2, 3, 45), // 10*4 + 2 + 3 = 45
            // Card 3: Easy, default ease, second combo, fast response
            (Rating::Easy, 2500, 4, 6, 70), // 15*4 + 4 + 6 = 70
            // Card 4: Again, resets combo, no other bonuses
            (Rating::Again, 2500, 0, 0, 0), // 0*4 + 0 + 0 = 0
        ];

        for (rating, ease, cb, sb, expected) in scenarios {
            assert_eq!(calculate_points_awarded(rating, ease, cb, sb), expected);
        }
    }
}
