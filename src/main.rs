use core::hash::Hasher;
use std::io::{
    Read as _,
    Seek as _,
};
use std::{
    fs::OpenOptions,
    io::{
        BufRead as _,
        BufReader,
    },
    // this trait import automatically assumes that it will work for Unix-based
    // operating systems and not Windows
    os::unix::process::CommandExt as _,
    path::{
        Path,
        PathBuf,
    },
    process::Command,
};

use nix::unistd::{
    geteuid,
    Pid,
    Uid,
};
use rand::{
    distributions::Alphanumeric,
    rngs::OsRng,
    Rng as _,
};
use rustc_hash::FxHasher;

const TARGET_DUMP_DIRECTORY: &str = "./target_dump/";

fn main() {
    if !geteuid().is_root() {
        eprintln!("User is not root. Have you tried running using sudo?");
        return;
    }

    {
        let converter = ConversionJob::new(
            PathBuf::from("./src/test_files/Coffee Run.webm"),
            get_sudo_invoker(),
        );

        converter.dump();
    }

    /*
    {
        let converter = ConversionJob::restore(&PathBuf::from("./target_dump"));

        converter.dump();
    }
    */
}

fn get_sudo_invoker() -> Uid {
    match std::env::var("SUDO_UID") {
        Ok(uid) => {
            match uid.parse::<u32>() {
                Ok(uid) => Uid::from_raw(uid),
                Err(e) => panic!("Cannot parse {} into i32: {:?}", uid, e),
            }
        },
        Err(e) => panic!("Cannot find the sudo-invoking user"),
    }
}

pub struct ConversionJob {
    //path: PathBuf,
    pid: Pid,
}

impl ConversionJob {
    pub fn new(
        path: PathBuf,
        uid: Uid,
    ) -> ConversionJob {
        let spawned = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-i")
            .arg(&path)
            .arg("-vn")
            .arg("-filter:a")
            .arg("loudnorm=print_format=json")
            .arg("-f")
            .arg("null")
            .arg("/dev/null")
            .uid(uid.into())
            .spawn()
            .unwrap();

        ConversionJob {
            //path,
            pid: Pid::from_raw(spawned.id() as i32),
        }
    }

    /// Dump the state of the program into a file
    pub fn dump(&self) {
        let target_folder = PathBuf::from(TARGET_DUMP_DIRECTORY);

        // create the temporary folder
        let temp_path = format!("./temp-{}/", generate_random_string());
        let temp_folder = PathBuf::from(temp_path);
        std::fs::create_dir(&temp_folder).unwrap();

        // pause the job
        let status = Command::new("criu")
            .arg("dump")
            .arg("--tree")
            .arg(&format!("{}", self.pid))
            .arg("--images-dir")
            .arg(&temp_folder)
            .arg("--shell-job")
            .arg("--leave-stopped")
            .output()
            .unwrap();

        if status.status.code() != Some(0) {
            panic!(
                "Job failed to be paused: {:?}",
                String::from_utf8(status.stderr)
            );
        }

        // continue the job
        nix::sys::signal::kill(self.pid, nix::sys::signal::Signal::SIGCONT);

        // remove the dump file, if it exists
        std::fs::remove_dir_all(&target_folder);

        // move the dump folder to the target folder
        match std::fs::rename(&temp_folder, &target_folder) {
            Err(e) => {
                panic!(
                    "Unable to move {} to {}: {}",
                    temp_folder.display(),
                    target_folder.display(),
                    e
                )
            },
            _ => {},
        }
    }

    pub fn restore(dump_path: &Path) -> ConversionJob {
        let target_folder = PathBuf::from(TARGET_DUMP_DIRECTORY);

        // create the file at which to write the PID into
        // TODO: see the todo in dump()
        let pid_filename = PathBuf::from("./pidfile.txt");

        // resume the job
        Command::new("criu")
            .arg("restore")
            .arg("--images-dir")
            .arg(target_folder)
            .arg("--shell-job")
            .arg("--pidfile")
            .arg(&pid_filename)
            .spawn();

        let pid_file =
            OpenOptions::new().read(true).open(&pid_filename).unwrap();
        let mut pid_str = String::new();
        BufReader::new(pid_file).read_line(&mut pid_str);
        pid_str.pop();

        let pid = pid_str.parse::<i32>().unwrap();
        let pid = Pid::from_raw(pid);

        ConversionJob {
            pid,
        }
    }
}

/// Generates a random string of 16 alphanumeric characters
fn generate_random_string() -> String {
    OsRng
        .sample_iter(Alphanumeric)
        .map(|u| u as char)
        .take(16)
        .collect::<String>()
}

/// Reads a file to get its hash.
///
/// The file is not read in its entirety to save computation time and I/O time.
/// Instead, the file is read this way:
/// ```
/// hasher(head(file, 65536))
/// hasher(tail(file, 65536))*
/// hasher(size(file))
/// ```
///
/// If the file's length is less than 65536 * 2, tail will not read the
/// overlapping bytes.
fn read_file_get_hash(
    path: &(impl AsRef<Path> + ?Sized)
) -> std::io::Result<u64> {
    use std::io::SeekFrom::{
        End,
        Start,
    };

    // get the file size
    let filesize = std::fs::metadata(path)?.len();

    // open the file for reading
    let mut file = OpenOptions::new().read(true).open(path)?;

    // NOTE: If in case of security concerns, feel free to replace the hash
    // function by something much more sensible.
    let mut hasher = FxHasher::default();

    // hash the first 65536 bytes
    {
        let mut buffer = vec![0u8; 65536];
        let bytes_read = file.read(&mut buffer)?;

        buffer.truncate(bytes_read);

        hasher.write(&buffer);
    }

    // hash the file size
    hasher.write_u64(filesize);

    // hash the last 65536 bytes. do not overlap if the file is too small
    if 65536 < filesize {
        if filesize < 65536 * 2 {
            file.seek(Start(65536));
        }
        else {
            file.seek(End(-65536));
        }

        let mut buffer = vec![0u8; 65536];
        let bytes_read = file.read(&mut buffer)?;

        buffer.truncate(bytes_read);

        hasher.write(&buffer);
    }

    Ok(hasher.finish())
}
