CREATE TABLE group_hashes (
    hash
        TEXT
        PRIMARY KEY
        NOT NULL,

    file_size
        INTEGER
        NOT NULL
);
CREATE INDEX group_hashes_hash_index ON group_hashes (hash);

CREATE TABLE files (
    hash
        TEXT
        NOT NULL,
    ordering
        INTEGER
        DEFAULT (0),
    file_path
        TEXT
        NOT NULL
        UNIQUE,

    UNIQUE (hash, ordering)
) STRICT;
CREATE INDEX files_hash_index ON files (hash)
