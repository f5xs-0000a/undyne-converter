use std::process::{
    Command,
    Stdio,
};

fn main() {
    match std::fs::remove_file("target/output_database.sqlite3") {
        _ => {},
    }

    let cat_process = Command::new("cat")
        .arg("src/schema.sql")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let sqlite_output = Command::new("sqlite3")
        .arg("target/output_database.sqlite3")
        .stdin(cat_process.stdout.unwrap())
        .output()
        .unwrap();

    let stderr = String::from_utf8(sqlite_output.stderr).unwrap();
    assert!(stderr.is_empty(), "{}", stderr);
}
