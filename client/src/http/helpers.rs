//! Helper functions for the HTTP client

use {
    crate::*,
    ::tracing::debug,
    serde_json::Value,
    std::{collections::HashSet, fs::File, io::Write, path::PathBuf},
    terminusdb_schema::{Instance, ToJson, ToTDBInstance, ToTDBSchema},
};

pub fn dedup_instances_by_id(instances: &mut Vec<&Instance>) {
    let mut seen_ids = HashSet::new();
    instances.retain(|item| {
        match &item.id {
            Some(id) => seen_ids.insert(id.clone()), // insert returns true if the value was not present in the set
            None => true,                            // keep items with None id
        }
    });
}

pub fn dedup_documents_by_id(values: &mut Vec<Value>) {
    let mut seen_ids = HashSet::new();
    values.retain(|value| {
        if let Some(id) = value.get("@id").and_then(|id| id.as_str()) {
            seen_ids.insert(id.to_string())
        } else {
            true
        }
    });
}

pub fn dump_failed_payload(payload: &str) {
    // Get the current datetime
    let current_datetime = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    // Define the log filename with the datetime
    let log_filename = format!("tdb-failed-request-{}.log.json", current_datetime);

    // Write the string to the log file
    let mut file = match File::create(&log_filename) {
        Ok(file) => file,
        Err(e) => panic!("Could not create file: {}", e),
    };

    match file.write_all(payload.as_bytes()) {
        Ok(_) => debug!(
            "Successfully dumped failed request payload to file {}",
            log_filename
        ),
        Err(e) => panic!("Could not write to file: {}", e),
    };
}

pub fn dump_schema<S: ToTDBSchema>() {
    // Get the current datetime
    let current_datetime = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    // Define the log filename with the datetime
    let log_filename = format!("tdb-failed-schema-{}.log.json", current_datetime);

    let schema_json = serde_json::Value::Array(
        S::to_schema_tree()
            .into_iter()
            .map(|s| s.to_json())
            .collect(),
    );

    let payload = serde_json::to_string_pretty(&schema_json).unwrap();

    // Write the string to the log file
    let mut file = match File::create(&log_filename) {
        Ok(file) => file,
        Err(e) => panic!("Could not create file: {}", e),
    };

    match file.write_all(payload.as_bytes()) {
        Ok(_) => debug!(
            "Successfully dumped failed request payload to file {}",
            log_filename
        ),
        Err(e) => panic!("Could not write to file: {}", e),
    };
}

pub fn dump_json(json: &Value) -> PathBuf {
    // Get the current datetime
    let current_datetime = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    // Define the log filename with the datetime
    let log_filename = format!("tdb-retrieved-json-{}.log.json", current_datetime);

    let payload = serde_json::to_string_pretty(json).unwrap();

    // Write the string to the log file
    let mut file = match File::create(&log_filename) {
        Ok(file) => file,
        Err(e) => panic!("Could not create file: {}", e),
    };

    match file.write_all(payload.as_bytes()) {
        Ok(_) => {
            debug!(
                "Successfully dumped success response to file {}",
                log_filename
            );
            PathBuf::from(log_filename)
        }
        Err(e) => panic!("Could not write to file: {}", e),
    }
}

pub fn format_id<T: ToTDBInstance>(id: &str) -> String {
    if id.contains("/") {
        id.to_string()
    } else {
        format!("{}/{}", T::schema_name(), id)
    }
}
