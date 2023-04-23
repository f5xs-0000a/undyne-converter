//! Contains methods required to perform the folder dance.
//!
//! Methods are moved here for convenience and to reduce line count for dump()
//! and restore()

use std::{
    fs::DirBuilder,
    fs::metadata,
    io::{
        Error as IoError,
        ErrorKind,
        Result as IoResult,
    },
    path::{
        Path,
        PathBuf,
    },
};

use rand::{
    distributions::Alphanumeric,
    rngs::OsRng,
    Rng as _,
};

/// Copies a path into a new name and returns the new name if it is moved.
///
/// # Panics
///
/// Panics if `path` cannot be moved.
///
/// TODO: this and rename should come from the same interface
pub fn copy_area_into_new_name(path: &Path) -> IoResult<PathBuf> {
    if !path.exists() {
        return Err(IoError::new(ErrorKind::NotFound, "path not found"));
    }

    if !path.is_dir() {
        return Err(IoError::new(ErrorKind::NotADirectory, "path is not a file"));
    }

    let new_path = randomize_until_vacant(path)?;
    std::process::Command::new("cp").arg("--archive").arg(path).arg(new_path.as_path()).output().unwrap();
    //DirBuilder::new()
    //    .recursive(true)
    //    .create(new_path.as_path())
    //    .and_then(|_| {
    //        std::fs::copy(path, new_path.as_path())
    //    })?;
    Ok(new_path)
}

/// Renames a path to get a new random name.
fn randomize(path: &Path) -> IoResult<PathBuf> {
    let random_string = generate_random_string();
    let file_name = path.file_name().unwrap().to_str().unwrap();
    let file_name_with_random_string = format!("{}-{}", random_string, file_name);
    Ok(path.with_file_name(file_name_with_random_string))
}

/// Renames a path to get a new random name that does not exist yet.
fn randomize_until_vacant(path: &Path) -> IoResult<PathBuf> {
    let mut new_path = randomize(path)?;

    while new_path.exists() {
        new_path = randomize(path)?;
    }

    Ok(new_path)
}

///
pub fn rename_to_appended_random_returning_new_name(
    path: &Path
) -> IoResult<PathBuf> {
    // generate the old name
    let pwd = path.parent().ok_or(IoError::new(
        ErrorKind::NotFound,
        "path does not have a parent",
    ))?;
    let name = path.file_name().ok_or(IoError::new(
        ErrorKind::NotFound,
        "path does not have a file name",
    ))?;

    // generate the new name
    let new_path = loop {
        let mut new_name = name.to_owned();
        new_name.push("-");
        new_name.push(generate_random_string());
        let new_path = pwd.join(new_name);

        // check if that name exists. if it doesn't exist, we'll use it
        // TODO: specificy which error this is
        if pwd.join(&new_path).metadata().is_err() {
            break new_path;
        }
    };

    std::fs::rename(path, &new_path).map(|_| new_path)
}

/// Generates a random string of 16 alphanumeric characters
pub fn generate_random_string() -> String {
    OsRng
        .sample_iter(Alphanumeric)
        .map(|u| u as char)
        .take(16)
        .collect::<String>()
}
