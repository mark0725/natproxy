use uuid::Uuid;

pub fn generate_uuid() -> String {
    let id = Uuid::new_v4().to_string();
    return id;
}