use std::fs;

pub fn parse_csv(path: &str) -> Result<Vec<(String, String)>, std::io::Error> {
    let content = fs::read_to_string(path)?;
    let mut rows = Vec::new();
    for line in content.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let mut cols = line.split(',');
        let key = cols.next().unwrap_or("").trim().to_string();
        let value = cols.next().unwrap_or("").trim().to_string();
        rows.push((key, value));
    }
    Ok(rows)
}
