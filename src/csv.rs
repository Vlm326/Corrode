use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Clone, Deserialize)]
pub struct StudentRow {
    pub name: String,
    pub surname: String,
    pub github: String,
}

pub fn parse_students(path: &str) -> Result<Vec<StudentRow>, Box<dyn std::error::Error>> {
    let mut reader = ::csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(path)?;
    let mut students = Vec::new();

    for result in reader.records() {
        let record = result?;
        let Some(name) = non_empty(record.get(0)) else {
            continue;
        };
        let Some(surname) = non_empty(record.get(1)) else {
            continue;
        };
        let Some(github) = non_empty(record.get(2)) else {
            continue;
        };

        students.push(StudentRow {
            name: name.to_string(),
            surname: surname.to_string(),
            github: normalize_github(github),
        });
    }

    debug!(path, count = students.len(), "CSV student rows parsed");
    Ok(students)
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    let value = value?.trim();
    (!value.is_empty()).then_some(value)
}

fn normalize_github(value: &str) -> String {
    let value = value.trim_end_matches('/');
    value
        .rsplit('/')
        .next()
        .unwrap_or(value)
        .trim_start_matches('@')
        .to_string()
}
