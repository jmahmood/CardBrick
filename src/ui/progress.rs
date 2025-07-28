// src/ui/progress.rs
// Progress viewer showing daily parameter choices and performance

use crate::storage::db::progress_path;
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone)]
pub struct DailyProgress {
    pub date: String,
    pub pack_sz: Option<usize>,
    pub rev_coef: Option<f64>,
    pub fail_k: Option<usize>,
    pub cards_studied: Option<usize>,
    pub points: Option<i64>,
    pub reward_bin: Option<bool>,
}

/// Load recent daily progress data for display
pub fn load_recent_progress(days_back: usize) -> Result<Vec<DailyProgress>, Box<dyn std::error::Error>> {
    let db_path = progress_path();
    let conn = Connection::open(&db_path)?;
    
    let mut stmt = conn.prepare(
        "SELECT date, pack_sz, rev_coef, fail_k, cards_studied, points, reward_bin 
         FROM daily_log 
         ORDER BY date DESC 
         LIMIT ?1"
    )?;
    
    let progress_iter = stmt.query_map([days_back], |row| {
        Ok(DailyProgress {
            date: row.get(0)?,
            pack_sz: row.get::<_, Option<i64>>(1)?.map(|x| x as usize),
            rev_coef: row.get(2)?,
            fail_k: row.get::<_, Option<i64>>(3)?.map(|x| x as usize),
            cards_studied: row.get::<_, Option<i64>>(4)?.map(|x| x as usize),
            points: row.get(5)?,
            reward_bin: row.get::<_, Option<i64>>(6)?.map(|x| x == 1),
        })
    })?;
    
    let mut progress = Vec::new();
    for item in progress_iter {
        progress.push(item?);
    }
    
    Ok(progress)
}

/// Get today's chosen parameters
pub fn get_todays_parameters() -> Result<Option<DailyProgress>, Box<dyn std::error::Error>> {
    let today = chrono::Utc::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();
    
    let db_path = progress_path();
    let conn = Connection::open(&db_path)?;
    
    let result = conn
        .prepare("SELECT date, pack_sz, rev_coef, fail_k, cards_studied, points, reward_bin FROM daily_log WHERE date = ?1")?
        .query_row([&today_str], |row| {
            Ok(DailyProgress {
                date: row.get(0)?,
                pack_sz: row.get::<_, Option<i64>>(1)?.map(|x| x as usize),
                rev_coef: row.get(2)?,
                fail_k: row.get::<_, Option<i64>>(3)?.map(|x| x as usize),
                cards_studied: row.get::<_, Option<i64>>(4)?.map(|x| x as usize),
                points: row.get(5)?,
                reward_bin: row.get::<_, Option<i64>>(6)?.map(|x| x == 1),
            })
        })
        .optional()?;
    
    Ok(result)
}

/// Format progress data for display
pub fn format_progress_table(progress: &[DailyProgress]) -> String {
    if progress.is_empty() {
        return "No progress data available".to_string();
    }
    
    let mut table = String::new();
    table.push_str("Date       | Pack | Rev.Coef | FailK | Cards | Points | Success\n");
    table.push_str("-----------|------|----------|-------|-------|--------|---------\n");
    
    for day in progress {
        let pack_sz = day.pack_sz.map_or("?".to_string(), |x| x.to_string());
        let rev_coef = day.rev_coef.map_or("?".to_string(), |x| format!("{:.1}", x));
        let fail_k = day.fail_k.map_or("?".to_string(), |x| x.to_string());
        let cards = day.cards_studied.map_or("?".to_string(), |x| x.to_string());
        let points = day.points.map_or("?".to_string(), |x| x.to_string());
        let success = day.reward_bin.map_or("?".to_string(), |x| if x { "" } else { "" }.to_string());
        
        table.push_str(&format!(
            "{} | {:4} | {:8} | {:5} | {:5} | {:6} | {}\n",
            day.date, pack_sz, rev_coef, fail_k, cards, points, success
        ));
    }
    
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_format_empty_progress() {
        let progress = vec![];
        let table = format_progress_table(&progress);
        assert!(table.contains("No progress data available"));
    }
    
    #[test]
    fn test_format_progress_table() {
        let progress = vec![
            DailyProgress {
                date: "2025-07-24".to_string(),
                pack_sz: Some(12),
                rev_coef: Some(2.0),
                fail_k: Some(5),
                cards_studied: Some(12),
                points: Some(36),
                reward_bin: Some(true),
            }
        ];
        
        let table = format_progress_table(&progress);
        assert!(table.contains("2025-07-24"));
        assert!(table.contains("12"));
        assert!(table.contains("2.0"));
        assert!(table.contains("5"));
        assert!(table.contains("36"));
        assert!(table.contains(""));
    }
}