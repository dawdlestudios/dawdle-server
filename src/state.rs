pub type DatabaseBackend = okv::backend::rocksdb::RocksDb;
pub type DB<K, V> = okv::Database<K, V, DatabaseBackend>;

pub struct User {
    username: String,
    password_hash: String,
}

pub struct State {
    users: DB<String, User>,
}
