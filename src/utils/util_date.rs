use chrono::{DateTime, Local};

pub fn get_date8()->String {
    let now: DateTime<Local> = Local::now();
    return now.format("%Y%m%d").to_string();
}

pub fn get_datetime14()->String {
    let now: DateTime<Local> = Local::now();
    return now.format("%Y%m%d%H%M%S").to_string();
}

pub fn get_date()->String {
    let now: DateTime<Local> = Local::now();
    return now.format("%Y-%m-%d").to_string();
}

pub fn get_datetime()->String {
    let now: DateTime<Local> = Local::now();
    return now.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
}