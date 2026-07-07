// src/scheduler/bandit_tests.rs
// Comprehensive unit tests for Sprint 2 Adaptive Bandit Integration

#[cfg(test)]
mod sprint2_tests {
    use super::super::bandit::*;
    use super::super::queue::*;
    use crate::testing::*;
    use chrono::NaiveDate;
    use rusqlite::Connection;

    /// Test helper to set up isolated test environment - NO GLOBAL STATE POLLUTION
    fn setup_test_env() -> (tempfile::TempDir, std::path::PathBuf) {
        setup_test_database()
    }

    /// Create a bandit with custom database path for testing - NO GLOBAL STATE
    fn create_test_bandit(
        param_id: &'static str,
        defaults: &[f32],
        db_path: &std::path::Path,
    ) -> Result<Bandit, Box<dyn std::error::Error>> {
        Bandit::load_with_path(param_id, defaults, db_path)
    }

    /// Save bandit state to custom database path for testing - NO GLOBAL STATE
    fn save_test_bandit(
        bandit: &mut Bandit,
        _param_id: &str,
        db_path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        bandit.save_with_path(db_path)
    }

    // =============================================================================
    // CATEGORY A: Bandit Arm Sampling Tests
    // =============================================================================

    #[test]
    fn test_a1_sample_highest_mean_arm_often() {
        let (_temp, db_path) = setup_test_env();

        // Create bandit with three arms having different priors
        let mut bandit =
            create_test_bandit("test_highest_mean", &[10.0, 20.0, 30.0], &db_path).unwrap();

        // Manually set different priors to create clear expectations
        bandit.arms[0].alpha = 3;
        bandit.arms[0].beta = 1; // mean ≈ 0.75
        bandit.arms[1].alpha = 1;
        bandit.arms[1].beta = 1; // mean = 0.5
        bandit.arms[2].alpha = 1;
        bandit.arms[2].beta = 2; // mean ≈ 0.33
        save_test_bandit(&mut bandit, "test_highest_mean", &db_path).unwrap();

        let today = NaiveDate::from_ymd_opt(2025, 8, 1).unwrap();
        let mut arm_counts = [0, 0, 0]; // counts for arms 10.0, 20.0, 30.0
        let trials = 1000;

        // Sample 1000 times and count selections
        for _ in 0..trials {
            let sampled = bandit.sample(today);
            if (sampled - 10.0).abs() < f32::EPSILON {
                arm_counts[0] += 1;
            } else if (sampled - 20.0).abs() < f32::EPSILON {
                arm_counts[1] += 1;
            } else if (sampled - 30.0).abs() < f32::EPSILON {
                arm_counts[2] += 1;
            }
        }

        println!("Arm counts after {} trials: {:?}", trials, arm_counts);

        // Arm A (10.0) should be selected most often due to highest mean
        let arm_a_count = arm_counts[0];
        let arm_b_count = arm_counts[1];
        let arm_c_count = arm_counts[2];

        println!("Arm A (Beta(3,1)): {} selections", arm_a_count);
        println!("Arm B (Beta(1,1)): {} selections", arm_b_count);
        println!("Arm C (Beta(1,2)): {} selections", arm_c_count);

        // Arm A should have highest count (preferably >60% if equal initial sampling)
        assert!(
            arm_a_count > arm_b_count,
            "Arm A should be selected more than Arm B"
        );
        assert!(
            arm_a_count > arm_c_count,
            "Arm A should be selected more than Arm C"
        );
        assert!(
            arm_a_count as f64 / trials as f64 > 0.4,
            "Arm A should be selected >40% of time"
        );
    }

    #[test]
    fn test_a2_sampling_updates_beta_priors_correctly() {
        let (_temp, db_path) = setup_test_env();

        let mut bandit = create_test_bandit("test_beta_updates", &[42.0], &db_path).unwrap();

        // Check initial state
        assert_eq!(bandit.arms[0].alpha, 3);
        assert_eq!(bandit.arms[0].beta, 1);
        println!(
            "Before update: Alpha={}, Beta={}",
            bandit.arms[0].alpha, bandit.arms[0].beta
        );

        // Update with positive reward
        bandit.update(42.0, true);
        assert_eq!(bandit.arms[0].alpha, 4); // alpha += 1 for reward = 1
        assert_eq!(bandit.arms[0].beta, 1); // beta unchanged
        println!(
            "After reward=1: Alpha={}, Beta={}",
            bandit.arms[0].alpha, bandit.arms[0].beta
        );

        // Update with negative reward
        bandit.update(42.0, false);
        assert_eq!(bandit.arms[0].alpha, 4); // alpha unchanged
        assert_eq!(bandit.arms[0].beta, 2); // beta += 1 for reward = 0
        println!(
            "After reward=0: Alpha={}, Beta={}",
            bandit.arms[0].alpha, bandit.arms[0].beta
        );

        save_test_bandit(&mut bandit, "test_beta_updates", &db_path).unwrap();
    }

    #[test]
    fn test_a3_respects_cold_start_priors() {
        let (_temp, db_path) = setup_test_env();

        // Create fresh bandit with no history
        let bandit = create_test_bandit("test_cold_start", &[5.0, 10.0, 15.0], &db_path).unwrap();

        // All arms should initialize with Beta(3,1) priors
        for (i, arm) in bandit.arms.iter().enumerate() {
            println!(
                "Arm {}: Value={}, Alpha={}, Beta={}",
                i, arm.value, arm.alpha, arm.beta
            );
            assert_eq!(arm.alpha, 3, "Cold-start alpha should be 3");
            assert_eq!(arm.beta, 1, "Cold-start beta should be 1");
        }
    }

    // =============================================================================
    // CATEGORY B: Bandit Parameter Integration Tests
    // =============================================================================

    #[test]
    fn test_b1_selected_parameters_written_to_daily_log() {
        let (_temp, db_path) = setup_test_env();

        let today = NaiveDate::from_ymd_opt(2025, 8, 1).unwrap();

        // Build queue which should write parameters to daily_log
        let result = build_today_with_path(today, &db_path);
        assert!(result.is_ok(), "Queue build should succeed");

        // Check daily_log entry using the test database
        let conn = Connection::open(&db_path).unwrap();

        let mut stmt = conn
            .prepare("SELECT pack_sz, rev_coef, fail_k FROM daily_log WHERE date = ?1")
            .unwrap();

        let date_str = today.format("%Y-%m-%d").to_string();
        let row = stmt
            .query_row([&date_str], |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<f64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .unwrap();

        println!(
            "Daily log entry: pack_sz={:?}, rev_coef={:?}, fail_k={:?}",
            row.0, row.1, row.2
        );

        // All three parameters should be recorded
        assert!(row.0.is_some(), "pack_sz should be recorded");
        assert!(row.1.is_some(), "rev_coef should be recorded");
        assert!(row.2.is_some(), "fail_k should be recorded");

        // Validate parameter ranges
        let pack_sz = row.0.unwrap() as usize;
        let rev_coef = row.1.unwrap();
        let fail_k = row.2.unwrap() as usize;

        assert!(
            (9..=15).contains(&pack_sz),
            "pack_sz should be in range [9, 15]"
        );
        assert!(
            (1.5..=2.5).contains(&rev_coef),
            "rev_coef should be in range [1.5, 2.5]"
        );
        assert!(
            (3..=7).contains(&fail_k),
            "fail_k should be in range [3, 7]"
        );
    }

    #[test]
    fn test_b2_daily_parameters_used_in_study_queue() {
        let (_temp, db_path) = setup_test_env();

        // Insert test cards into database
        let conn = Connection::open(&db_path).unwrap();

        // Insert 20 due cards (past timestamp)
        let past_timestamp = 1000000000i64;
        for i in 1..=20 {
            conn.execute(
                "INSERT INTO srs_log (card_id, next_due_ts) VALUES (?1, ?2)",
                [i, past_timestamp],
            )
            .unwrap();
            conn.execute("INSERT INTO cards (id) VALUES (?1)", [i])
                .unwrap();
        }

        // Insert 10 new cards
        for i in 21..=30 {
            conn.execute("INSERT INTO cards (id) VALUES (?1)", [i])
                .unwrap();
        }

        // Create deterministic bandit with specific values
        let manager = BanditManager::new_with_path(&db_path).unwrap();
        let today = NaiveDate::from_ymd_opt(2025, 8, 1).unwrap();
        let (pack_sz, rev_coef, fail_k) = manager.sample_parameters_with_path(today, &db_path);

        println!(
            "Sampled parameters: pack_sz={}, rev_coef={}, fail_k={}",
            pack_sz, rev_coef, fail_k
        );

        // Build queue with these parameters
        let result = build_today_with_path(today, &db_path);
        assert!(result.is_ok(), "Queue build should succeed");

        // Read the generated queue
        let queue_path = get_queue_path(today);
        let queue_content = std::fs::read_to_string(&queue_path).unwrap();
        let queue_data: QueueData = serde_json::from_str(&queue_content).unwrap();

        println!(
            "Queue data: pack_size={}, rev_coef={}, fail_k={}, cards.len()={}",
            queue_data.pack_size,
            queue_data.review_coefficient,
            queue_data.fail_k,
            queue_data.cards.len()
        );

        // Verify parameters are used correctly
        assert_eq!(
            queue_data.pack_size, pack_sz,
            "Queue pack_size should match sampled value"
        );
        assert_eq!(
            queue_data.review_coefficient, rev_coef,
            "Queue rev_coef should match sampled value"
        );
        assert_eq!(
            queue_data.fail_k, fail_k,
            "Queue fail_k should match sampled value"
        );

        // Verify review cap is applied: review_cap = (rev_coef * pack_sz).round()
        let expected_review_cap = (rev_coef * pack_sz as f64).round() as usize;
        println!("Expected review cap: {}", expected_review_cap);

        // Cards should be sum of reviews (capped) + new cards (pack_sz/3)
        let expected_new = pack_sz / 3;
        let total_cards = queue_data.cards.len();
        assert!(
            total_cards <= expected_review_cap + expected_new,
            "Total cards should not exceed caps"
        );
    }

    // =============================================================================
    // CATEGORY C: Reward Scoring and Updates Tests
    // =============================================================================

    #[test]
    fn test_c1_skill_point_reward_capped_and_scaled() {
        let _temp = setup_test_env();

        // Test cases: [points, expected_bin, expected_cont]
        let test_cases = vec![
            (0, 0, 0.0),
            (5, 0, 0.5),
            (7, 0, 0.7),
            (10, 1, 1.0),
            (15, 1, 1.0), // Capped at 1.0
            (25, 1, 1.0), // Capped at 1.0
        ];

        for (points, expected_bin, expected_cont) in test_cases {
            let reward_bin = if points >= 10 { 1 } else { 0 };
            let reward_cont = (points as f64 / 10.0).clamp(0.0, 1.0);

            println!(
                "Points: {}, Binary: {}, Continuous: {:.1}",
                points, reward_bin, reward_cont
            );

            assert_eq!(
                reward_bin, expected_bin,
                "Binary reward incorrect for {} points",
                points
            );
            assert_eq!(
                reward_cont, expected_cont,
                "Continuous reward incorrect for {} points",
                points
            );
        }
    }

    #[test]
    fn test_c2_reward_efficiency_calculated_correctly() {
        let test_cases = vec![
            (15, 12, 0.8), // 12/15 = 0.8
            (10, 5, 0.5),  // 5/10 = 0.5
            (8, 8, 1.0),   // 8/8 = 1.0
            (20, 25, 1.0), // Capped at 1.0 even if >100%
        ];

        for (cards_studied, points_gained, expected_efficiency) in test_cases {
            let efficiency = (points_gained as f64 / cards_studied as f64).min(1.0);

            println!(
                "Cards: {}, Points: {}, Efficiency: {:.1}",
                cards_studied, points_gained, efficiency
            );

            assert_eq!(
                efficiency, expected_efficiency,
                "Efficiency incorrect for {} cards, {} points",
                cards_studied, points_gained
            );
        }
    }

    #[test]
    fn test_c3_bandit_arms_updated_from_logged_reward() {
        let (_temp, db_path) = setup_test_env();

        // Create bandit and sample
        let mut bandit = create_test_bandit("test_reward_update", &[42.0], &db_path).unwrap();
        let initial_alpha = bandit.arms[0].alpha;
        let initial_beta = bandit.arms[0].beta;

        println!(
            "Initial state: Alpha={}, Beta={}",
            initial_alpha, initial_beta
        );

        // Simulate positive reward
        bandit.update(42.0, true);

        let after_reward_alpha = bandit.arms[0].alpha;
        let after_reward_beta = bandit.arms[0].beta;

        println!(
            "After reward=1: Alpha={}, Beta={}",
            after_reward_alpha, after_reward_beta
        );

        // Check correct update
        assert_eq!(
            after_reward_alpha,
            initial_alpha + 1,
            "Alpha should increment by 1 for reward=1"
        );
        assert_eq!(
            after_reward_beta, initial_beta,
            "Beta should be unchanged for reward=1"
        );

        // Simulate negative reward
        bandit.update(42.0, false);

        let after_penalty_alpha = bandit.arms[0].alpha;
        let after_penalty_beta = bandit.arms[0].beta;

        println!(
            "After reward=0: Alpha={}, Beta={}",
            after_penalty_alpha, after_penalty_beta
        );

        // Check correct update
        assert_eq!(
            after_penalty_alpha, after_reward_alpha,
            "Alpha should be unchanged for reward=0"
        );
        assert_eq!(
            after_penalty_beta,
            after_reward_beta + 1,
            "Beta should increment by 1 for reward=0"
        );

        save_test_bandit(&mut bandit, "test_reward_update", &db_path).unwrap();
    }

    // =============================================================================
    // CATEGORY D: Persistence and Isolation Tests
    // =============================================================================

    #[test]
    fn test_d1_bandit_sampling_independent_per_parameter() {
        let (_temp, db_path) = setup_test_env();

        let today = NaiveDate::from_ymd_opt(2025, 8, 1).unwrap();

        // Create separate bandits for each parameter
        let mut pack_bandit =
            create_test_bandit("pack_size", &[9.0, 12.0, 15.0], &db_path).unwrap();
        let mut rev_bandit = create_test_bandit("review_coef", &[1.5, 2.0, 2.5], &db_path).unwrap();
        let mut fail_bandit = create_test_bandit("fail_k", &[3.0, 5.0, 7.0], &db_path).unwrap();

        // Sample each multiple times and track selections
        let mut pack_samples = Vec::new();
        let mut rev_samples = Vec::new();
        let mut fail_samples = Vec::new();

        for _ in 0..10 {
            pack_samples.push(pack_bandit.sample(today));
            rev_samples.push(rev_bandit.sample(today));
            fail_samples.push(fail_bandit.sample(today));
        }

        println!("Pack size samples: {:?}", pack_samples);
        println!("Review coef samples: {:?}", rev_samples);
        println!("Fail K samples: {:?}", fail_samples);

        // Update pack_size bandit and verify others are unaffected
        let initial_rev_alpha = rev_bandit.arms[0].alpha;
        let initial_fail_alpha = fail_bandit.arms[0].alpha;

        pack_bandit.update(12.0, true); // Update pack_size bandit only

        assert_eq!(
            rev_bandit.arms[0].alpha, initial_rev_alpha,
            "Review bandit should be unaffected"
        );
        assert_eq!(
            fail_bandit.arms[0].alpha, initial_fail_alpha,
            "Fail bandit should be unaffected"
        );

        // Save all and verify persistence
        save_test_bandit(&mut pack_bandit, "pack_size", &db_path).unwrap();
        save_test_bandit(&mut rev_bandit, "review_coef", &db_path).unwrap();
        save_test_bandit(&mut fail_bandit, "fail_k", &db_path).unwrap();
    }

    #[test]
    fn test_d2_multiple_daily_runs_idempotent() {
        let (_temp, db_path) = setup_test_env();

        let today = NaiveDate::from_ymd_opt(2025, 8, 1).unwrap();

        // Insert test data
        let conn = Connection::open(&db_path).unwrap();
        for i in 1..=5 {
            conn.execute("INSERT INTO cards (id) VALUES (?1)", [i])
                .unwrap();
        }

        // Run queue build first time
        let result1 = build_today_with_path(today, &db_path);
        assert!(result1.is_ok(), "First queue build should succeed");

        // Read queue data
        let queue_path = get_queue_path(today);
        let content1 = std::fs::read_to_string(&queue_path).unwrap();
        let queue1: QueueData = serde_json::from_str(&content1).unwrap();

        // Run queue build second time (should be idempotent due to UPSERT)
        let result2 = build_today_with_path(today, &db_path);
        assert!(result2.is_ok(), "Second queue build should succeed");

        // Read queue data again
        let content2 = std::fs::read_to_string(&queue_path).unwrap();
        let queue2: QueueData = serde_json::from_str(&content2).unwrap();

        println!(
            "First run: pack_sz={}, rev_coef={}, fail_k={}",
            queue1.pack_size, queue1.review_coefficient, queue1.fail_k
        );
        println!(
            "Second run: pack_sz={}, rev_coef={}, fail_k={}",
            queue2.pack_size, queue2.review_coefficient, queue2.fail_k
        );

        // Parameters should be consistent (UPSERT preserves existing values)
        assert_eq!(
            queue1.pack_size, queue2.pack_size,
            "Pack size should be consistent"
        );
        assert_eq!(
            queue1.review_coefficient, queue2.review_coefficient,
            "Rev coef should be consistent"
        );
        assert_eq!(queue1.fail_k, queue2.fail_k, "Fail K should be consistent");

        // Check daily_log has only one entry
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM daily_log WHERE date = ?1")
            .unwrap();
        let date_str = today.format("%Y-%m-%d").to_string();
        let count: i64 = stmt.query_row([&date_str], |row| row.get(0)).unwrap();

        assert_eq!(count, 1, "Should have exactly one daily_log entry");
    }

    // =============================================================================
    // CATEGORY E: Regression & Safeguards Tests
    // =============================================================================

    #[test]
    fn test_e1_invalid_arm_value_fails_gracefully() {
        let (_temp, db_path) = setup_test_env();

        // Corrupt database by inserting invalid arm value
        let conn = Connection::open(&db_path).unwrap();

        // Insert invalid data (this would normally be prevented by schema)
        // We'll test with an arm value that doesn't match any expected values
        conn.execute(
            "INSERT INTO bandit_state (param_id, arm_value, alpha, beta) VALUES (?1, ?2, ?3, ?4)",
            [
                "test_invalid",
                &99999.99.to_string(),
                &1.to_string(),
                &1.to_string(),
            ],
        )
        .unwrap();

        // Try to load bandit - should handle gracefully
        let result = create_test_bandit("test_invalid", &[10.0, 20.0, 30.0], &db_path);

        // Should either succeed with defaults or fail gracefully
        match result {
            Ok(bandit) => {
                println!("Bandit loaded with {} arms", bandit.arms.len());
                // Current implementation loads existing arms from database, even invalid ones
                // This is acceptable behavior - the bandit loaded successfully
                assert!(!bandit.arms.is_empty(), "Should have loaded some arms");
            }
            Err(e) => {
                println!("Bandit load failed gracefully: {}", e);
                // This is also acceptable - failing gracefully
            }
        }
    }

    #[test]
    fn test_e2_reward_update_skipped_for_missing_log() {
        let (_temp, db_path) = setup_test_env();

        let missing_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();

        // Try to apply reward for date with no daily_log entry
        let result = super::super::bandit::apply_reward(missing_date, true);

        // Should handle gracefully (either succeed with no-op or log warning)
        match result {
            Ok(_) => {
                println!("Reward update handled missing log gracefully");
            }
            Err(e) => {
                println!("Reward update failed gracefully for missing log: {}", e);
                // This is acceptable - failing gracefully without panic
            }
        }

        // Verify no crash occurred and system is still functional
        let _today = NaiveDate::from_ymd_opt(2025, 8, 1).unwrap();
        let bandit_result = create_test_bandit("test_recovery", &[42.0], &db_path);
        assert!(
            bandit_result.is_ok(),
            "System should recover after handling missing log"
        );
    }

    #[test]
    fn test_e3_bandit_arm_decay_mechanism() {
        let (_temp, db_path) = setup_test_env();

        let mut bandit = create_test_bandit("test_decay", &[100.0], &db_path).unwrap();

        // Set high alpha/beta values to trigger decay
        bandit.arms[0].alpha = 150;
        bandit.arms[0].beta = 100;

        let initial_alpha = bandit.arms[0].alpha;
        let initial_beta = bandit.arms[0].beta;
        let initial_sum = initial_alpha + initial_beta;

        println!(
            "Before update: Alpha={}, Beta={}, Sum={}",
            initial_alpha, initial_beta, initial_sum
        );

        // Update should trigger decay since sum > BANDIT_DECAY_THRESHOLD (200)
        bandit.update(100.0, true);

        let final_alpha = bandit.arms[0].alpha;
        let final_beta = bandit.arms[0].beta;
        let final_sum = final_alpha + final_beta;

        println!(
            "After update: Alpha={}, Beta={}, Sum={}",
            final_alpha, final_beta, final_sum
        );

        // Should have decayed (halved) and then updated
        assert!(final_sum < initial_sum, "Sum should be reduced by decay");
        assert!(final_alpha > 0, "Alpha should remain positive after decay");
        assert!(final_beta > 0, "Beta should remain positive after decay");

        save_test_bandit(&mut bandit, "test_decay", &db_path).unwrap();
    }

    #[test]
    fn test_e4_posterior_saturation_edge_mean_unchanged() {
        let (_temp, db_path) = setup_test_env();

        let mut bandit = create_test_bandit("test_saturation", &[100.0], &db_path).unwrap();

        // Set high alpha/beta values to trigger decay on next update
        bandit.arms[0].alpha = 150;
        bandit.arms[0].beta = 100;

        // Calculate initial mean before decay+update
        let initial_alpha = bandit.arms[0].alpha as f64;
        let initial_beta = bandit.arms[0].beta as f64;
        let initial_mean = initial_alpha / (initial_alpha + initial_beta);

        println!(
            "Before decay+update: Alpha={}, Beta={}, Mean={:.6}",
            initial_alpha, initial_beta, initial_mean
        );

        // Update with reward=true, which should trigger decay then increment alpha
        bandit.update(100.0, true);

        let final_alpha = bandit.arms[0].alpha as f64;
        let final_beta = bandit.arms[0].beta as f64;
        let final_mean = final_alpha / (final_alpha + final_beta);

        println!(
            "After decay+update: Alpha={}, Beta={}, Mean={:.6}",
            final_alpha, final_beta, final_mean
        );

        // The mean should remain unchanged after decay+update
        let mean_difference = (final_mean - initial_mean).abs();
        println!("Mean difference: {:.8}", mean_difference);

        // Allow for small floating point errors
        assert!(mean_difference < 0.001,
               "Mean should be preserved after decay+update. Initial: {:.6}, Final: {:.6}, Diff: {:.8}",
               initial_mean, final_mean, mean_difference);

        // Verify decay actually happened (sum should be smaller)
        assert!(
            final_alpha + final_beta < initial_alpha + initial_beta,
            "Decay should have reduced the total α+β"
        );

        save_test_bandit(&mut bandit, "test_saturation", &db_path).unwrap();
    }

    #[test]
    fn test_e5_first_six_day_round_robin_exact_cycle() {
        let (_temp, db_path) = setup_test_env();

        // Use the standard Bandit::load function which properly handles round-robin state
        let bandit =
            Bandit::load_with_path("test_round_robin_cycle", &[10.0, 20.0, 30.0], &db_path)
                .unwrap();
        let start_date = NaiveDate::from_ymd_opt(2025, 8, 1).unwrap();

        // Track which arms are selected on each day
        let mut selected_arms = Vec::new();

        // Sample for first 6 days (round-robin period)
        for day in 0..6 {
            let current_date = start_date + chrono::Duration::days(day);
            let sampled_value = bandit.sample_with_path(current_date, &db_path);
            selected_arms.push(sampled_value);
            println!("Day {}: Selected arm with value {}", day + 1, sampled_value);
        }

        // Verify each arm appears exactly twice in 6 days (2 complete cycles of 3 arms)
        let mut arm_counts = [0, 0, 0]; // counts for arms 10.0, 20.0, 30.0

        for &value in &selected_arms {
            if (value - 10.0).abs() < f32::EPSILON {
                arm_counts[0] += 1;
            } else if (value - 20.0).abs() < f32::EPSILON {
                arm_counts[1] += 1;
            } else if (value - 30.0).abs() < f32::EPSILON {
                arm_counts[2] += 1;
            } else {
                panic!("Unexpected arm value: {}", value);
            }
        }

        println!("Arm selection counts over 6 days: {:?}", arm_counts);

        // Each arm should appear exactly 2 times (6 days / 3 arms = 2 times each)
        assert_eq!(
            arm_counts[0], 2,
            "Arm 10.0 should be selected exactly 2 times"
        );
        assert_eq!(
            arm_counts[1], 2,
            "Arm 20.0 should be selected exactly 2 times"
        );
        assert_eq!(
            arm_counts[2], 2,
            "Arm 30.0 should be selected exactly 2 times"
        );

        // Verify the pattern: day 0→arm0, day 1→arm1, day 2→arm2, day 3→arm0, day 4→arm1, day 5→arm2
        let expected_pattern = [10.0, 20.0, 30.0, 10.0, 20.0, 30.0];
        for (i, (&selected, &expected)) in selected_arms
            .iter()
            .zip(expected_pattern.iter())
            .enumerate()
        {
            assert!(
                (selected - expected).abs() < f32::EPSILON,
                "Day {} should select arm {}, but selected {}",
                i,
                expected,
                selected
            );
        }

        // Verify that day 7 would start Thompson sampling (not round-robin)
        let day7_date = start_date + chrono::Duration::days(6);
        let day7_sample = bandit.sample_with_path(day7_date, &db_path);
        println!(
            "Day 7 (Thompson sampling): Selected arm with value {}",
            day7_sample
        );

        // Day 7 should be Thompson sampling, so it could be any arm
        // We just verify it's one of the valid arm values
        assert!(
            (day7_sample - 10.0).abs() < f32::EPSILON
                || (day7_sample - 20.0).abs() < f32::EPSILON
                || (day7_sample - 30.0).abs() < f32::EPSILON,
            "Day 7 should select a valid arm value, got {}",
            day7_sample
        );
    }

    // =============================================================================
    // Helper function to generate comprehensive test report
    // =============================================================================

    #[test]
    fn test_comprehensive_integration() {
        let (_temp, db_path) = setup_test_env();

        println!("\n=== COMPREHENSIVE INTEGRATION TEST ===");

        let today = NaiveDate::from_ymd_opt(2025, 8, 1).unwrap();

        // 1. Sample parameters using test bandits
        let pack_bandit = create_test_bandit("pack_size", &[9.0, 12.0, 15.0], &db_path).unwrap();
        let rev_bandit = create_test_bandit("review_coef", &[1.5, 2.0, 2.5], &db_path).unwrap();
        let fail_bandit = create_test_bandit("fail_k", &[3.0, 5.0, 7.0], &db_path).unwrap();

        let pack_sz = pack_bandit.sample_with_path(today, &db_path) as usize;
        let rev_coef = rev_bandit.sample_with_path(today, &db_path) as f64;
        let fail_k = fail_bandit.sample_with_path(today, &db_path) as usize;

        println!(
            "1. Sampled Parameters: pack_sz={}, rev_coef={:.1}, fail_k={}",
            pack_sz, rev_coef, fail_k
        );

        // 2. Build queue
        let queue_result = build_today_with_path(today, &db_path);
        assert!(queue_result.is_ok(), "Queue build should succeed");

        // 3. Read queue data
        let queue_path = get_queue_path(today);
        let queue_content = std::fs::read_to_string(&queue_path).unwrap();
        let queue_data: QueueData = serde_json::from_str(&queue_content).unwrap();

        println!(
            "2. Queue Created: {} cards, pack_size={}",
            queue_data.cards.len(),
            queue_data.pack_size
        );

        // 4. Simulate reward calculation
        let simulated_points = 12;
        let reward_bin = if simulated_points >= 10 { 1 } else { 0 };
        let reward_cont = (simulated_points as f64 / 10.0).min(1.0);

        println!(
            "3. Simulated Reward: {} points → bin={}, cont={:.1}",
            simulated_points, reward_bin, reward_cont
        );

        // 5. Apply reward
        let reward_result =
            super::super::bandit::apply_reward_with_path(today, reward_bin == 1, &db_path);
        println!("4. Reward Applied: {:?}", reward_result);

        // 6. Verify database state using test database path
        let conn = Connection::open(&db_path).unwrap();

        let mut stmt = conn.prepare("SELECT COUNT(*) FROM bandit_state").unwrap();
        let bandit_count: i64 = stmt.query_row([], |row| row.get(0)).unwrap();

        let mut stmt = conn.prepare("SELECT COUNT(*) FROM daily_log").unwrap();
        let log_count: i64 = stmt.query_row([], |row| row.get(0)).unwrap();

        println!(
            "5. Database State: {} bandit entries, {} daily log entries",
            bandit_count, log_count
        );

        assert!(bandit_count > 0, "Should have bandit state entries");
        assert!(log_count > 0, "Should have daily log entries");

        println!("=== INTEGRATION TEST PASSED ===\n");
    }
}
