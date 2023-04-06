use core::hash::Hasher;
use std::{
    ffi::OsStr,
    io::{
        Read as _,
        Result as IoResult,
        Seek as _,
    },
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

use nix::{
    sys::signal::{
        kill,
        Signal,
    },
    unistd::{
        geteuid,
        Pid,
        Uid,
    },
};
use rand::{
    distributions::Alphanumeric,
    rngs::OsRng,
    Rng as _,
};
use rustc_hash::FxHasher;

const TARGET_DUMP_DIRECTORY: &str = "./target_dump/";

// TODO: implement LowerHex
struct JobId(u64);

fn main() {
    if !geteuid().is_root() {
        eprintln!("User is not root. Have you tried running using sudo?");
        return;
    }

    let path = "./src/test_files/Coffee Run.webm";

    {
        let maybe_converter = ConversionJob::new(path, get_sudo_invoker());
        let converter = match maybe_converter {
            Ok(c) => c,
            Err(e) => panic!("Cannot read {}: {:?}", path, e),
        };

        converter.dump();
    }

    /*
    {
        let hash = match read_file_get_hash(path) {
            Ok(h) => h,
            Err(e) => panic!("Cannot read {}: {:?}", path, e),
        };
        let restore_path = format!("./{:x}/", hash);
        let converter = ConversionJob::restore(&restore_path);

        converter.dump();
    }
    */
}

impl Drop for ConversionJob {
    fn drop(&mut self) {
        kill(self.pid, Signal::SIGKILL);
    }
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
    job_id: JobId,
}

impl ConversionJob {
    pub fn new(
        path: &(impl AsRef<Path> + AsRef<OsStr> + ?Sized),
        uid: Uid,
    ) -> IoResult<ConversionJob> {
        let job_id = JobId(read_file_get_hash(path)?);

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

        Ok(ConversionJob {
            //path,
            pid: Pid::from_raw(spawned.id() as i32),
            job_id,
        })
    }

    /// Dump the state of the program into a file
    pub fn dump(&self) {
        let target_folder = format!("./{:x}", self.job_id.0);

        // create the temporary folder
        let temp_path =
            format!("./{:x}-{}/", self.job_id.0, generate_random_string());
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
        kill(self.pid, Signal::SIGCONT);

        // remove the dump file, if it exists
        std::fs::remove_dir_all(&target_folder);

        // move the dump folder to the target folder
        match std::fs::rename(&temp_folder, &target_folder) {
            Err(e) => {
                panic!(
                    "Unable to move {} to {}: {}",
                    temp_folder.display(),
                    target_folder,
                    e
                )
            },
            _ => {},
        }
    }

    pub fn restore(dump_path: &(impl AsRef<Path> + ?Sized)) -> ConversionJob {
        // create the file at which to write the PID into
        // TODO: see the todo in dump()
        let pid_filename = PathBuf::from("./pidfile.txt");

        // resume the job
        Command::new("criu")
            .arg("restore")
            .arg("--images-dir")
            .arg(dump_path.as_ref())
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

        // TODO: this should be obtained from a database
        let job_id = JobId(12345);

        ConversionJob {
            pid,
            job_id,
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
fn read_file_get_hash(path: &(impl AsRef<Path> + ?Sized)) -> IoResult<u64> {
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
