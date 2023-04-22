//! Contains methods required to perform the folder dance.
//!
//! Methods are moved here for convenience and to reduce line count for dump()
//! and restore()

use std::{
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

/// Moves a path into a new name and returns the new name if it is moved.
///
/// # Panics
///
/// Panics if `path` cannot be moved.
///
/// TODO: this and rename should come from the same interface
pub fn copy_area_into_new_name(path: &Path) -> Option<PathBuf> {
    // from `path`, create a new randomized name. a randomized name.
    let target_path = path.to_owned();

    // check if the old target exists
    let old_path_exists = match metadata(&path).map(|m| m.is_dir()) {
        Ok(true) => true,
        Ok(false) => {
            if std::fs::remove_file(&target_path).is_err() {}

            eprintln!(
                "A file labelled {} has been deleted.",
                target_path.display()
            );
            return None;
        },
        Err(e) if e.kind() == ErrorKind::NotFound => return None,
        Err(e) => {
            panic!(
                "Failed to move directory {}: {:?}",
                target_path.display(),
                e
            )
        },
    };

    Some(target_path)
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
