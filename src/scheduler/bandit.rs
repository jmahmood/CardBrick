// src/scheduler/bandit.rs
// Thompson Sampling Multi-Armed Bandit for adaptive parameter tuning

use crate::config::{BANDIT_DECAY_THRESHOLD, ROUND_ROBIN_DAYS};
use crate::storage::db::progress_path;
use chrono::NaiveDate;
use rand::{thread_rng, Rng};
use rand_distr::{Beta, Distribution};
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone)]
pub struct Arm {
    pub value: f32,
    pub alpha: u32,
    pub beta: u32,
}

impl Arm {
    fn new(value: f32) -> Self {
        Self {
            value,
            alpha: 3, // Optimistic prior
            beta: 1,  // Optimistic prior
        }
    }

    fn sample_probability(&self, rng: &mut impl Rng) -> f64 {
        let beta_dist = Beta::new(self.alpha as f64, self.beta as f64)
            .expect("Beta distribution parameters invalid");
        beta_dist.sample(rng)
    }

    fn update(&mut self, reward: bool) {
        if reward {
            self.alpha += 1;
        } else {
            self.beta += 1;
        }

        // Apply decay if posterior becomes too large
        if self.alpha + self.beta > BANDIT_DECAY_THRESHOLD {
            self.alpha /= 2;
            self.beta /= 2;
            if self.alpha == 0 {
                self.alpha = 1;
            }
            if self.beta == 0 {
                self.beta = 1;
            }
        }
    }
}

#[derive(Debug)]
pub struct Bandit {
    pub param_id: &'static str,
    pub arms: Vec<Arm>,
    pub dirty: bool,
}

impl Bandit {
    pub fn load(
        param_id: &'static str,
        defaults: &[f32],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let db_path = progress_path();
        Self::load_with_path(param_id, defaults, &db_path)
    }

    pub fn load_with_path(
        param_id: &'static str,
        defaults: &[f32],
        db_path: &std::path::Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let conn = Connection::open(db_path)?;

        // Load existing arms or create with defaults
        let mut stmt =
            conn.prepare("SELECT arm_value, alpha, beta FROM bandit_state WHERE param_id = ?1")?;

        let arm_rows: Result<Vec<_>, _> = stmt
            .query_map([param_id], |row| {
                Ok((
                    row.get::<_, f64>(0)? as f32,
                    row.get::<_, i64>(1)? as u32,
                    row.get::<_, i64>(2)? as u32,
                ))
            })?
            .collect();

        let arms = match arm_rows {
            Ok(rows) if !rows.is_empty() => {
                // Load existing arms
                rows.into_iter()
                    .map(|(value, alpha, beta)| Arm { value, alpha, beta })
                    .collect()
            }
            _ => {
                // Create new arms with defaults
                let new_arms: Vec<Arm> = defaults.iter().map(|&value| Arm::new(value)).collect();

                // Save to database
                for arm in &new_arms {
                    conn.execute(
                        "INSERT OR REPLACE INTO bandit_state (param_id, arm_value, alpha, beta) VALUES (?1, ?2, ?3, ?4)",
                        [param_id, &arm.value.to_string(), &arm.alpha.to_string(), &arm.beta.to_string()],
                    )?;
                }

                new_arms
            }
        };

        let bandit = Self {
            param_id,
            arms,
            dirty: false,
        };

        Ok(bandit)
    }

    pub fn sample(&self, today: NaiveDate) -> f32 {
        let db_path = progress_path();
        self.sample_with_path(today, &db_path)
    }

    pub fn sample_with_path(&self, today: NaiveDate, db_path: &std::path::Path) -> f32 {
        // Check if we're in round-robin cold start phase
        if let Ok(should_use_rr) = self.should_use_round_robin_with_path(today, db_path) {
            if should_use_rr {
                return self.round_robin_sample_with_path(today, db_path);
            }
        }

        // Thompson sampling: sample from each arm's posterior and pick the best
        let mut rng = thread_rng();
        let mut best_arm_idx = 0;
        let mut best_sample = 0.0;

        for (idx, arm) in self.arms.iter().enumerate() {
            let sample = arm.sample_probability(&mut rng);
            if sample > best_sample {
                best_sample = sample;
                best_arm_idx = idx;
            }
        }

        self.arms[best_arm_idx].value
    }

    pub fn update(&mut self, chosen_value: f32, reward: bool) {
        // Find the arm that was chosen and update it
        if let Some(arm) = self
            .arms
            .iter_mut()
            .find(|arm| (arm.value - chosen_value).abs() < f32::EPSILON)
        {
            arm.update(reward);
            self.dirty = true;
        }
    }

    pub fn save(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = progress_path();
        self.save_with_path(&db_path)
    }

    pub fn save_with_path(
        &mut self,
        db_path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.dirty {
            return Ok(());
        }

        let conn = Connection::open(db_path)?;

        for arm in &self.arms {
            conn.execute(
                "INSERT OR REPLACE INTO bandit_state (param_id, arm_value, alpha, beta) VALUES (?1, ?2, ?3, ?4)",
                [self.param_id, &arm.value.to_string(), &arm.alpha.to_string(), &arm.beta.to_string()],
            )?;
        }

        self.dirty = false;
        Ok(())
    }

    #[allow(dead_code)]
    fn should_use_round_robin(&self, today: NaiveDate) -> Result<bool, Box<dyn std::error::Error>> {
        let db_path = progress_path();
        self.should_use_round_robin_with_path(today, &db_path)
    }

    fn should_use_round_robin_with_path(
        &self,
        today: NaiveDate,
        db_path: &std::path::Path,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let conn = Connection::open(db_path)?;

        // Get the first day this bandit was used
        let key = format!("first_day_{}", self.param_id);
        let first_day: Option<String> = conn
            .prepare("SELECT value FROM meta WHERE key = ?1")?
            .query_row([&key], |row| row.get::<_, String>(0))
            .optional()?;

        let first_day = match first_day {
            Some(day_str) => NaiveDate::parse_from_str(&day_str, "%Y-%m-%d")?,
            None => {
                // This is the first time - record today as the first day
                let today_str = today.format("%Y-%m-%d").to_string();
                conn.execute(
                    "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                    [&key, &today_str],
                )?;
                today
            }
        };

        let days_since_first = (today - first_day).num_days();
        Ok(days_since_first < ROUND_ROBIN_DAYS as i64)
    }

    #[allow(dead_code)]
    fn round_robin_sample(&self, today: NaiveDate) -> f32 {
        let db_path = progress_path();
        self.round_robin_sample_with_path(today, &db_path)
    }

    fn round_robin_sample_with_path(&self, today: NaiveDate, db_path: &std::path::Path) -> f32 {
        if let Ok(conn) = Connection::open(db_path) {
            let key = format!("first_day_{}", self.param_id);
            if let Ok(Some(first_day_str)) = conn
                .prepare("SELECT value FROM meta WHERE key = ?1")
                .and_then(|mut stmt| {
                    stmt.query_row([&key], |row| row.get::<_, String>(0))
                        .optional()
                })
            {
                if let Ok(first_day) = NaiveDate::parse_from_str(&first_day_str, "%Y-%m-%d") {
                    let days_since_first = (today - first_day).num_days() as usize;
                    let arm_idx = days_since_first % self.arms.len();
                    return self.arms[arm_idx].value;
                }
            }
        }

        // Fallback to first arm if DB operations fail
        self.arms[0].value
    }
}

impl Drop for Bandit {
    fn drop(&mut self) {
        if self.dirty {
            let _ = self.save(); // Ignore errors on drop
        }
    }
}

/// Manages all three bandits for parameter sampling
pub struct BanditManager {
    pack_size_bandit: Bandit,
    review_coef_bandit: Bandit,
    fail_k_bandit: Bandit,
}

impl BanditManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let db_path = progress_path();
        Self::new_with_path(&db_path)
    }

    pub fn new_with_path(db_path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            pack_size_bandit: Bandit::load_with_path("pack_size", &[9.0, 12.0, 15.0], db_path)?,
            review_coef_bandit: Bandit::load_with_path("review_coef", &[1.5, 2.0, 2.5], db_path)?,
            fail_k_bandit: Bandit::load_with_path("fail_k", &[3.0, 5.0, 7.0], db_path)?,
        })
    }

    pub fn sample_parameters(&self, today: NaiveDate) -> (usize, f64, usize) {
        let db_path = progress_path();
        self.sample_parameters_with_path(today, &db_path)
    }

    pub fn sample_parameters_with_path(
        &self,
        today: NaiveDate,
        db_path: &std::path::Path,
    ) -> (usize, f64, usize) {
        let pack_sz = self.pack_size_bandit.sample_with_path(today, db_path) as usize;
        let rev_coef = self.review_coef_bandit.sample_with_path(today, db_path) as f64;
        let fail_k = self.fail_k_bandit.sample_with_path(today, db_path) as usize;

        (pack_sz, rev_coef, fail_k)
    }

    pub fn update_with_reward(
        &mut self,
        pack_sz: usize,
        rev_coef: f64,
        fail_k: usize,
        reward: bool,
    ) {
        self.pack_size_bandit.update(pack_sz as f32, reward);
        self.review_coef_bandit.update(rev_coef as f32, reward);
        self.fail_k_bandit.update(fail_k as f32, reward);
    }

    pub fn save_all(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = progress_path();
        self.save_all_with_path(&db_path)
    }

    pub fn save_all_with_path(
        &mut self,
        db_path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.pack_size_bandit.save_with_path(db_path)?;
        self.review_coef_bandit.save_with_path(db_path)?;
        self.fail_k_bandit.save_with_path(db_path)?;
        Ok(())
    }
}

/// Apply reward to the bandits that were used on the given date
pub fn apply_reward(date: NaiveDate, reward: bool) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = progress_path();
    apply_reward_with_path(date, reward, &db_path)
}

/// Apply reward with explicit database path - NO GLOBAL STATE
pub fn apply_reward_with_path(
    date: NaiveDate,
    reward: bool,
    db_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get the parameters that were used on this date from daily_log
    let conn = Connection::open(db_path)?;

    let date_str = date.format("%Y-%m-%d").to_string();
    let mut stmt =
        conn.prepare("SELECT pack_sz, rev_coef, fail_k FROM daily_log WHERE date = ?1")?;

    if let Ok(row) = stmt.query_row([&date_str], |row| {
        Ok((
            row.get::<_, Option<i64>>(0)?,
            row.get::<_, Option<f64>>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    }) {
        let mut manager = BanditManager::new_with_path(db_path)?;

        if let (Some(pack_sz), Some(rev_coef), Some(fail_k)) = (row.0, row.1, row.2) {
            manager.update_with_reward(pack_sz as usize, rev_coef, fail_k as usize, reward);
            manager.save_all_with_path(db_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_arm_creation() {
        let arm = Arm::new(12.0);
        assert_eq!(arm.value, 12.0);
        assert_eq!(arm.alpha, 3);
        assert_eq!(arm.beta, 1);
    }

    #[test]
    fn test_arm_update() {
        let mut arm = Arm::new(12.0);

        // Test positive reward
        arm.update(true);
        assert_eq!(arm.alpha, 4);
        assert_eq!(arm.beta, 1);

        // Test negative reward
        arm.update(false);
        assert_eq!(arm.alpha, 4);
        assert_eq!(arm.beta, 2);
    }

    #[test]
    fn test_arm_decay() {
        let mut arm = Arm::new(12.0);
        arm.alpha = 150;
        arm.beta = 100;

        arm.update(true); // This should trigger decay
        assert!(arm.alpha < 150);
        assert!(arm.beta < 100);
        assert!(arm.alpha > 0);
        assert!(arm.beta > 0);
    }

    #[test]
    fn test_round_robin_exact_cycle() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Initialize database with required schema
        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS bandit_state (
            param_id TEXT NOT NULL,
            arm_value REAL NOT NULL,
            alpha INTEGER NOT NULL,
            beta INTEGER NOT NULL,
            PRIMARY KEY (param_id, arm_value)
        )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
            [],
        )
        .unwrap();
        drop(conn);

        let bandit = Bandit::load_with_path("rr_test", &[10.0, 20.0, 30.0], &db_path).unwrap();
        let start_date = NaiveDate::from_ymd_opt(2025, 7, 24).unwrap();

        // First 3 days should cycle through arms in exact order
        let expected = [10.0, 20.0, 30.0];
        for i in 0..3 {
            let date = start_date + chrono::Duration::days(i);
            let sample = bandit.sample_with_path(date, &db_path);
            assert!(
                (sample - expected[i as usize]).abs() < f32::EPSILON,
                "Day {} expected {}, got {}",
                i,
                expected[i as usize],
                sample
            );
        }
    }

    #[test]
    fn test_bandit_manager() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        // Initialize database with required schema
        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS bandit_state (
            param_id TEXT NOT NULL,
            arm_value REAL NOT NULL,
            alpha INTEGER NOT NULL,
            beta INTEGER NOT NULL,
            PRIMARY KEY (param_id, arm_value)
        )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
            [],
        )
        .unwrap();
        drop(conn);

        let mut manager = BanditManager::new_with_path(&db_path).unwrap();
        let today = NaiveDate::from_ymd_opt(2025, 7, 24).unwrap();

        let (pack_sz, rev_coef, fail_k) = manager.sample_parameters_with_path(today, &db_path);
        assert!((9..=15).contains(&pack_sz));
        assert!((1.5..=2.5).contains(&rev_coef));
        assert!((3..=7).contains(&fail_k));

        // Test update
        manager.update_with_reward(pack_sz, rev_coef, fail_k, true);
        manager.save_all_with_path(&db_path).unwrap();
    }

    #[cfg(feature = "sim")]
    #[test]
    fn test_bandit_convergence() {
        let temp_dir = tempdir().unwrap();
        std::env::set_var("HOME", temp_dir.path());

        // Simulate 3 arms with known success rates
        let true_success_rates = [0.2, 0.6, 0.8];
        let arm_values = [10.0, 20.0, 30.0];

        let mut bandit = Bandit::load("convergence_test", &arm_values).unwrap();
        let start_date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

        let mut best_arm_count = 0;
        let total_rounds = 500;

        for round in 0..total_rounds {
            let date = start_date + chrono::Duration::days(round);
            let chosen_value = bandit.sample(date);
            let chosen_idx = arm_values
                .iter()
                .position(|&x| (x - chosen_value).abs() < f32::EPSILON)
                .unwrap();

            // Simulate reward based on true success rate
            let reward = thread_rng().gen::<f64>() < true_success_rates[chosen_idx];
            bandit.update(chosen_value, reward);

            // Track if best arm (30.0, with 0.8 success rate) was chosen
            if (chosen_value - 30.0).abs() < f32::EPSILON {
                best_arm_count += 1;
            }
        }

        bandit.save().unwrap();

        // After convergence, best arm should be chosen at least 70% of the time
        let best_arm_ratio = best_arm_count as f64 / total_rounds as f64;
        assert!(
            best_arm_ratio >= 0.7,
            "Best arm chosen {}% < 70%",
            best_arm_ratio * 100.0
        );
    }
}
