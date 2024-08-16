use argon2::PasswordHasher;
use eyre::Result;
use std::fmt::{self, Debug, Formatter};

pub fn to_time(timestamp: i64) -> Result<time::OffsetDateTime> {
    Ok(time::OffsetDateTime::from_unix_timestamp(timestamp)?)
}

pub fn is_valid_username(username: &str) -> bool {
    !username.is_empty()
        && username.len() < 32
        && username
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn is_valid_project_path(path: &str) -> bool {
    // check for leading slash, multiple slashes, and ..
    !path.is_empty()
        && path.len() < 64
        && !path.starts_with('/')
        && !path.contains("//")
        && !path.contains("..")
        && path.chars().all(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || c == '/'
                || c == '.'
                || c == '-'
                || c == '_'
        })
}

pub struct RingBuffer<T> {
    buffer: Vec<T>,
    capacity: usize,
    start: usize,
    size: usize,
}

impl<T: Debug> Debug for RingBuffer<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "RingBuffer {{ buffer: [")?;
        for (i, item) in self.buffer.iter().enumerate() {
            if i == self.start {
                write!(f, "({:?}), ", item)?;
            } else {
                write!(f, "{:?}, ", item)?;
            }
        }
        write!(
            f,
            "], capacity: {}, start: {}, size: {} }}",
            self.capacity, self.start, self.size
        )
    }
}

impl<T: Clone + Debug> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        RingBuffer {
            buffer: Vec::with_capacity(capacity),
            capacity,
            start: 0,
            size: 0,
        }
    }

    pub fn push(&mut self, item: T) {
        if self.size < self.capacity {
            // If buffer is not full, just push the item.
            self.buffer.push(item);
            self.size += 1;
        } else {
            // If buffer is full, replace the item at the correct position.
            let end = (self.start + self.size) % self.capacity;
            self.buffer[end] = item;
            self.start = (self.start + 1) % self.capacity;
        }
    }

    pub fn to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.size);
        for i in 0..self.size {
            let idx = (self.start + i) % self.capacity;
            result.push(self.buffer[idx].clone());
        }
        result
    }
}

pub fn hash_pw(password: &str) -> eyre::Result<String> {
    Ok(argon2::Argon2::default()
        .hash_password(
            password.as_bytes(),
            &argon2::password_hash::SaltString::generate(&mut rand::rngs::OsRng),
        )?
        .to_string())
}

pub fn valid_public_key(key: &str) -> bool {
    let Ok(k) = ssh_key::PublicKey::from_openssh(key) else {
        return false;
    };

    matches! {
        k.algorithm(),
        ssh_key::Algorithm::Ed25519
    }
}
