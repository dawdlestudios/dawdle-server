pub fn is_valid_username(username: &str) -> bool {
    !username.is_empty()
        && username.len() < 32
        && username.chars().all(|c| c.is_ascii_alphanumeric())
}
