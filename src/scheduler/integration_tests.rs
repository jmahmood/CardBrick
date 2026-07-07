// src/scheduler/integration_tests.rs
// 30-day simulation harness for Sprint 3 integration testing

#[cfg(test)]
mod tests {
    use crate::config::DAILY_GOAL_POINTS;
    use crate::scheduler::bandit;
    use crate::scheduler::points::*;
    use crate::scheduler::sm2::*;
    use crate::storage::schema::*;
    use chrono::{Datelike, Duration, NaiveDate};
    use rusqlite::Connection;
    use tempfile::tempdir;

    /// Simulates a 30-day study period with realistic user behavior
    #[test]
    fn test_30_day_study_simulation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("simulation.db");

        // Initialize database with proper schema
        let conn = Connection::open(&db_path).unwrap();
        setup_test_database(&conn).unwrap();
        drop(conn);

        // Simulation parameters
        let start_date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let end_date = start_date + Duration::days(29); // 30 days total (inclusive)
        let mut current_date = start_date;

        // Track simulation metrics
        let mut daily_points = Vec::new();
        let mut daily_cards_studied = Vec::new();
        let mut goal_achievement_count = 0;
        let mut total_cards_studied = 0;

        println!("🧪 Starting 30-day CardBrick study simulation");
        println!("   Start: {}, End: {}", start_date, end_date);

        while current_date <= end_date {
            let day_number = (current_date - start_date).num_days() + 1;

            // Simulate daily study session
            let session_result = simulate_daily_session(current_date, &db_path);

            match session_result {
                Ok((points_earned, cards_studied, goal_achieved)) => {
                    daily_points.push(points_earned);
                    daily_cards_studied.push(cards_studied);
                    total_cards_studied += cards_studied;

                    if goal_achieved {
                        goal_achievement_count += 1;
                    }

                    println!(
                        "📅 Day {}: {} points, {} cards, goal: {}",
                        day_number,
                        points_earned,
                        cards_studied,
                        if goal_achieved { "✓" } else { "✗" }
                    );
                }
                Err(e) => {
                    eprintln!("❌ Day {} simulation failed: {}", day_number, e);
                }
            }

            current_date += Duration::days(1);
        }

        // Analyze simulation results
        let avg_daily_points = daily_points.iter().sum::<i32>() as f64 / daily_points.len() as f64;
        let avg_daily_cards =
            daily_cards_studied.iter().sum::<i32>() as f64 / daily_cards_studied.len() as f64;
        let goal_achievement_rate = goal_achievement_count as f64 / 30.0;
        let max_daily_points = daily_points.iter().max().unwrap_or(&0);
        let min_daily_points = daily_points.iter().min().unwrap_or(&0);

        println!("\n📊 30-Day Simulation Results:");
        println!("   Total cards studied: {}", total_cards_studied);
        println!("   Average daily points: {:.1}", avg_daily_points);
        println!("   Average daily cards: {:.1}", avg_daily_cards);
        println!(
            "   Goal achievement rate: {:.1}% ({}/30 days)",
            goal_achievement_rate * 100.0,
            goal_achievement_count
        );
        println!(
            "   Points range: {} - {}",
            min_daily_points, max_daily_points
        );

        // Assertions to validate simulation results
        assert!(
            total_cards_studied >= 30,
            "Should study at least 1 card per day on average"
        );
        assert!(
            avg_daily_points >= 5.0,
            "Should earn reasonable points daily"
        );
        assert!(
            goal_achievement_count >= 5,
            "Should achieve goal on multiple days"
        );
        assert!(
            *max_daily_points >= DAILY_GOAL_POINTS,
            "Should reach daily goal at least once"
        );
        assert!(daily_points.len() == 30, "Should have data for all 30 days");

        // Test bandit system integration (optional)
        let _bandit_learned = validate_bandit_learning(&db_path, start_date).unwrap();

        println!("✅ 30-day simulation completed successfully!");
    }

    /// Simulates a single day's study session with realistic behavior
    fn simulate_daily_session(
        date: NaiveDate,
        db_path: &std::path::Path,
    ) -> Result<(i32, i32, bool), Box<dyn std::error::Error>> {
        let timestamp_base = date.and_hms_opt(14, 30, 0).unwrap().and_utc().timestamp(); // 2:30 PM study time

        // Simulate studying 8-15 cards per day (realistic range)
        let cards_to_study = simulate_daily_card_count(date);
        let mut total_points = 0;
        let mut combo = 0;

        for card_index in 0..cards_to_study {
            let card_id = (date.ordinal() as i64 * 100) + card_index as i64; // Generate unique card IDs
            let timestamp = timestamp_base + (card_index as i64 * 30); // 30 seconds between cards

            // Simulate realistic rating distribution
            let rating = simulate_realistic_rating(card_index, cards_to_study);
            let ease_factor = simulate_card_ease_factor(card_id);

            // Apply SM-2 rating
            apply_rating_with_path(card_id, rating, timestamp, db_path)?;

            // Calculate points using the same logic as the real application
            let response_time = simulate_response_time();
            let (new_combo, cb) = combo_bonus(combo, rating);
            combo = new_combo;

            let sb = speed_bonus(response_time);
            let pa = calculate_points_awarded(rating, ease_factor, cb, sb);

            total_points += pa;

            // Record daily rating for progress tracking
            record_daily_rating_with_path(card_id, rating, timestamp, db_path)?;
        }

        let goal_achieved = total_points >= DAILY_GOAL_POINTS;

        // Simulate bandit reward application
        bandit::apply_reward_with_path(date, goal_achieved, db_path)?;

        Ok((total_points, cards_to_study, goal_achieved))
    }

    /// Simulates realistic daily card count (8-15 cards, with some variation)
    fn simulate_daily_card_count(date: NaiveDate) -> i32 {
        let day_of_week = date.weekday().num_days_from_monday();
        let base_cards = 12;

        // Weekend might have slightly fewer cards
        let weekend_modifier = if day_of_week >= 5 { -2 } else { 0 };

        // Add some randomness based on date
        let date_seed = date.ordinal() % 7;
        let random_modifier = match date_seed {
            0..=2 => -1,
            3..=4 => 0,
            _ => 1,
        };

        (base_cards + weekend_modifier + random_modifier).clamp(8, 15)
    }

    /// Simulates realistic rating distribution (mostly Good, some Easy/Hard, occasional Again)
    fn simulate_realistic_rating(card_index: i32, total_cards: i32) -> Rating {
        // Use card index as pseudo-random seed
        let seed = (card_index * 7 + total_cards * 3) % 100;

        match seed {
            0..=5 => Rating::Again,  // 6% - realistic failure rate
            6..=15 => Rating::Hard,  // 10% - struggling but not failing
            16..=25 => Rating::Easy, // 10% - mastered cards
            _ => Rating::Good,       // 74% - normal recall (majority)
        }
    }

    /// Simulates card ease factors (mostly default 2500, with some variation)
    fn simulate_card_ease_factor(card_id: i64) -> u32 {
        let seed = (card_id % 10) as u32;
        match seed {
            0..=1 => 1400, // Difficult cards (20%)
            2..=3 => 2000, // Below average (20%)
            4..=6 => 2500, // Default ease (30%)
            7..=8 => 2800, // Above average (20%)
            _ => 3200,     // Mastered cards (10%)
        }
    }

    /// Simulates response times (mostly 4-8 seconds, some faster/slower)
    fn simulate_response_time() -> f32 {
        // Use a simple distribution for response times
        let base_time = 6.0; // Average response time
        let variation = (std::ptr::addr_of!(base_time) as usize % 5) as f32; // Simple pseudo-randomness
        (base_time + variation - 2.0).clamp(2.0, 12.0)
    }

    /// Records daily rating with explicit database path
    fn record_daily_rating_with_path(
        card_id: i64,
        rating: Rating,
        timestamp: i64,
        db_path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = Connection::open(db_path)?;
        let date = chrono::DateTime::from_timestamp(timestamp, 0)
            .unwrap()
            .date_naive();
        let date_str = date.format("%Y-%m-%d").to_string();
        let rating_str = match rating {
            Rating::Again => "Again",
            Rating::Hard => "Hard",
            Rating::Good => "Good",
            Rating::Easy => "Easy",
        };

        conn.execute(
            "INSERT INTO daily_ratings (card_id, rating, timestamp, date) VALUES (?1, ?2, ?3, ?4)",
            [
                card_id.to_string(),
                rating_str.to_string(),
                timestamp.to_string(),
                date_str,
            ],
        )?;

        Ok(())
    }

    /// Validates that the bandit system has learned from the simulation
    fn validate_bandit_learning(
        db_path: &std::path::Path,
        start_date: NaiveDate,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let conn = Connection::open(db_path)?;

        // Check if bandit tables have been updated
        let bandit_entries: i64 = conn.query_row(
            "SELECT COUNT(*) FROM bandit_log WHERE date >= ?1",
            [start_date.format("%Y-%m-%d").to_string()],
            |row| row.get(0),
        )?;

        // Should have at least some bandit learning entries (relaxed for test)
        Ok(bandit_entries >= 5)
    }

    /// Sets up test database with all required tables
    fn setup_test_database(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        create_test_database_tables(conn)?;
        Ok(())
    }

    /// Test SM-2 algorithm behavior over extended period
    #[test]
    fn test_sm2_interval_progression() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("sm2_test.db");
        let conn = Connection::open(&db_path).unwrap();
        setup_test_database(&conn).unwrap();
        drop(conn);

        let card_id = 12345;
        let base_timestamp = 1640995200; // 2022-01-01 00:00:00 UTC
        let mut intervals: Vec<i64> = Vec::new();
        let mut ease_factors: Vec<f64> = Vec::new();

        // Simulate perfect recall sequence (all Good ratings)
        for i in 0..10 {
            let timestamp = base_timestamp + (i * 24 * 60 * 60); // Daily progression
            apply_rating_with_path(card_id, Rating::Good, timestamp, &db_path).unwrap();

            // Read back the state
            let conn = Connection::open(&db_path).unwrap();
            let (interval, ease_factor): (i64, f64) = conn
                .query_row(
                    "SELECT interval, ease_factor FROM srs_log WHERE card_id = ?1",
                    [card_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();

            intervals.push(interval);
            ease_factors.push(ease_factor);
            drop(conn);
        }

        println!("📈 SM-2 Interval Progression (10 Good ratings):");
        for (i, (interval, ease)) in intervals.iter().zip(ease_factors.iter()).enumerate() {
            println!(
                "   Rating {}: interval={} days, ease={:.2}",
                i + 1,
                interval,
                ease
            );
        }

        // Validate SM-2 behavior
        assert!(intervals[0] == 1, "First interval should be 1 day");
        assert!(
            intervals[1] == 6,
            "Second interval should be 6 days (SM-2 standard)"
        );
        assert!(
            intervals[2] > intervals[1],
            "Intervals should generally increase"
        );
        assert!(
            intervals[9] > intervals[0],
            "Long-term intervals should be much larger"
        );
        assert!(
            (ease_factors[0] - 2.5).abs() < 0.001,
            "Initial ease factor should be 2.5"
        );

        println!("✅ SM-2 algorithm behaves correctly over extended period");
    }

    /// Test points system edge cases and scaling
    #[test]
    fn test_points_system_comprehensive() {
        // Test extreme scenarios
        let test_cases = vec![
            // (rating, ease_factor, combo, speed_bonus, expected_min, expected_max, description)
            (Rating::Again, 1300, 0, 0, 0, 20, "Failed easy card"),
            (Rating::Easy, 3000, 20, 6, 80, 120, "Perfect mastered card"),
            (
                Rating::Good,
                2500,
                10,
                3,
                50,
                70,
                "Good default card with combo",
            ),
            (Rating::Hard, 1400, 0, 0, 5, 15, "Hard difficult card"),
        ];

        println!("🧮 Points System Comprehensive Test:");
        for (rating, ease_factor, combo, speed_bonus, min_expected, max_expected, desc) in
            test_cases
        {
            let points = calculate_points_awarded(rating, ease_factor, combo, speed_bonus);
            println!(
                "   {}: {} points (expected {}-{})",
                desc, points, min_expected, max_expected
            );

            assert!(
                points >= min_expected && points <= max_expected,
                "Points {} outside expected range {}-{} for {}",
                points,
                min_expected,
                max_expected,
                desc
            );
        }

        println!("✅ Points system handles edge cases correctly");
    }
}
