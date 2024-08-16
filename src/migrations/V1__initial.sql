create table users (
    username text primary key not null,
    password_hash text not null,
    role text,
    created_at integer not null default (strftime('%s', 'now'))
)

create table user_public_keys (
    username text not null,
    name text not null,
    public_key text not null,
    primary key (username, public_key),
    foreign key (username) references users (username) on delete cascade
)

create table sessions (
    session_token text primary key not null,
    username text not null,
    created_at integer not null default (strftime('%s', 'now')),
    last_used_at integer not null default (strftime('%s', 'now')),
    logged_out boolean not null default false,
    foreign key (username) references users (username) on delete cascade
)

create table applications (
    application_id text primary key not null,
    requested_username text not null,
    email text not null,
    about text not null,
    approved boolean not null default false,
    claimed boolean not null default false,
    claim_token text,
    created_at integer not null default (strftime('%s', 'now'))
)
