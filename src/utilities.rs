use sqlx::Database;

////////////////////////////////////////////////////////////////////////////////

pub type DB = Sqlite;
pub type DB_CONN = <DB as Database>::Connection;

pub fn check_and_read_database_file(path: &Path) -> Option<DB_CONN> {
    eprintln!("Checking if the database exists...");
    if !self.database.exists() {
        eprintln!("Database file does not exist yet. Creating one...");

        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&args.db);

        match file {
            Ok(f) => f.write_all(
                include_bytes!("../target/output_database.sqlite3")
            ),
            _ => {
                eprintln!("Cannot open database file for writing");
                return None;
            }
        }
    }

    else if self.database.is_file() {
        eprintln!("Database path is not a file. Aborting.");
        return None;
    } 
    
    // if the file isn't a database file
    else if unimplemented!() {
        eprintln!("Database path is not a valid sqlite database. Aborting.");
        return None;
    }

    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .foreign_keys(false)
        .filename(&args.db);

    // connection to database? that's an open file.
    let connection =
        <asmr_downloader::queries::DB as Database>::Connection::connect(path)
        .await
        .ok();

    Some(connection)
}
