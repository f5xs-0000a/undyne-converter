use core::{
    hash::Hasher,
    mem::drop,
};
use std::{
    ffi::OsStr,
    io::{
        ErrorKind,
        Read as _,
        Result as IoResult,
        Seek as _,
    },
    process::Stdio,
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

use clap::Parser;
use libc::{
    getpid,
    setsid,
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

#[derive(PartialEq, Parser)]
enum App {
    Dump,
    Restore,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
struct JobId(u64);

impl core::fmt::LowerHex for JobId {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter,
    ) -> Result<(), core::fmt::Error> {
        self.0.fmt(f)
    }
}

fn main() {
    use App::*;

    if !geteuid().is_root() {
        eprintln!("User is not root. Have you tried running using sudo?");
        return;
    }

    let mode = App::parse();

    let path = "./src/test_files/Coffee Run.webm";

    match mode {
        Dump => {
            let maybe_converter = ConversionJob::new(path, get_sudo_invoker());

            let converter = match maybe_converter {
                Ok(c) => c,
                Err(e) => panic!("Cannot read {}: {:?}", path, e),
            };

            converter.dump().unwrap();
        },

        Restore => {
            let hash = match read_file_get_hash(path) {
                Ok(h) => h,
                Err(e) => panic!("Cannot read {}: {:?}", path, e),
            };
            let restore_path = format!("./{:x}/", hash);

            let _converter = ConversionJob::restore(&restore_path);
            //converter.dump();
        },
    };
}

impl Drop for ConversionJob {
    fn drop(&mut self) {
        // kill without regards
        if let Some(pid) = self.pid {
            drop(kill(pid, Signal::SIGKILL));
        }
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
        Err(e) => panic!("Cannot find the sudo-invoking user: {:?}", e),
    }
}

fn get_sudo_invoker_name() -> String {
    match std::env::var("SUDO_USER") {
        Ok(uid) => uid,
        Err(e) => panic!("Cannot find the sudo-invoking user: {:?}", e),
    }
}

pub struct ConversionJob {
    /// The path to the media being converted.
    path: PathBuf,

    /// The ID of the process.
    ///
    /// Obtained from the system. If this is a `None`, then the object is a
    /// dummy.
    pid: Option<Pid>,

    /// The ID of the job.
    ///
    /// Generated by this program.
    job_id: JobId,
}

impl ConversionJob {
    pub fn new(
        file_path: &(impl AsRef<Path> + AsRef<OsStr> + ?Sized),
        uid: Uid,
    ) -> IoResult<ConversionJob> {
        let job_id = JobId(read_file_get_hash(file_path)?);

        // create a dummy Job object for now
        let dummy = ConversionJob {
            job_id,
            path: PathBuf::new(),
            pid: None,
        };

        let spawned = unsafe {
            Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-i")
            .arg(&dummy.path)
            .arg("-vn")
            .arg("-filter:a")
            .arg("loudnorm=print_format=json")
            .arg("-f")
            .arg("null")
            .arg("/dev/null")
            // set the user ID into the caller's
            .uid(uid.into())
            // we have to detach the process from the tty so criu doesn't
            // have to complain that the process we're trying to restore does
            // not have a tty included
            // to do that, set all stdin, stdout, and stderr to null
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // criu also complains, if the process restored is not a shell job
            // (related above), and the process is not its own session leader
            .pre_exec(|| {
                setsid();
                // Make the command the leader of the new session
                libc::setpgid(0, getpid());
                Ok(())
            })
            .spawn()
            .unwrap()
        };

        let real_job = ConversionJob {
            pid: Some(Pid::from_raw(spawned.id() as i32)),
            job_id,
            path: <_ as AsRef<Path>>::as_ref(file_path).to_path_buf(),
        };

        // create the folders
        std::fs::create_dir_all(real_job.working_path())?;

        Ok(real_job)
    }

    /// Obtain the base path.
    ///
    /// The hierarchy of paths go like this:
    /// base
    /// ├─ job
    /// │  ├─ working
    /// │  ├─ saved_working
    /// │  └─ dump
    /// ├─ job
    /// ...
    ///
    /// The base path is relative to the invoking user's home directory.
    fn base_path(&self) -> PathBuf {
        let mut path = PathBuf::from("/");
        path.push("home");
        path.push(get_sudo_invoker_name());
        path.push(".criu");
        path
    }

    /// Obtain the path of the job.
    ///
    /// The hierarchy of paths go like this:
    /// base
    /// ├─ job
    /// │  ├─ working
    /// │  ├─ saved_working
    /// │  └─ dump
    /// ├─ job
    /// ...
    ///
    /// The job path is the directory where the files and directories required
    /// to perform and restore a job is located.
    fn job_path(&self) -> PathBuf {
        let mut path = self.base_path();
        path.push(format!("{:x}", self.job_id.0));
        path
    }

    /// Obtain the working path of a job.
    ///
    /// The hierarchy of paths go like this:
    /// base
    /// ├─ job
    /// │  ├─ working
    /// │  ├─ saved_working
    /// │  └─ dump
    /// ├─ job
    /// ...
    ///
    /// The working path is the directory where the files being read and/or
    /// written by the job's process are located.
    fn working_path(&self) -> PathBuf {
        let mut path = self.job_path();
        path.push("working");
        path
    }

    /// Obtain the saved-working path of a job.
    ///
    /// The hierarchy of paths go like this:
    /// base
    /// ├─ job
    /// │  ├─ working
    /// │  ├─ saved_working
    /// │  └─ dump
    /// ├─ job
    /// ...
    ///
    /// The saved-working path is similar to the working path, except that
    /// when the process state is dumped into the dump path, so to are the files
    /// in the working path copied into saved_working.
    /// written by the job's process are located.
    fn saved_working_path(&self) -> PathBuf {
        let mut path = self.job_path();
        path.push("saved_working");
        path
    }

    /// obtain the dumping path of the job
    ///
    /// the hierarchy of paths go like this:
    /// base
    /// ├─ job
    /// │  ├─ working
    /// │  ├─ saved_working
    /// │  └─ dump
    /// ├─ job
    /// ...
    ///
    /// The dump path is the directory where the files required to restore the
    /// job is located.
    fn dump_path(&self) -> PathBuf {
        let mut path = self.job_path();
        path.push("dump");
        path
    }

    /// Performed upon finishing a job.
    fn on_finish(&self) {}

    /// Generates a name for a temporary dumping directory without creating
    /// said directory.
    fn create_temp_dump_dir_name(&self) -> PathBuf {
        let mut path = self.job_path();
        path.push(format!("dump-{}", generate_random_string()));
        path
    }

    /// Create a temporary dumping directory and
    /// This method will be mostly used by the dump method.
    fn create_temp_dump_dir(&self) -> IoResult<PathBuf> {
        let temp_dir = self.create_temp_dump_dir_name();
        std::fs::create_dir_all(&temp_dir)?;
        Ok(temp_dir)
    }

    /// Dump the state of the program into a file
    pub fn dump(&self) -> IoResult<()> {
        let target_dump_path = self.dump_path();

        // create the temporary folder to put the new dump into
        let temp_path = self.create_temp_dump_dir()?;
        std::fs::create_dir(&temp_path);

        // TODO: how do you check if the process really still exists?

        // pause the job
        let status = Command::new("criu")
            .arg("dump")
            .arg("--tree")
            .arg(&format!("{}", self.pid.as_ref().unwrap()))
            .arg("--images-dir")
            .arg(&temp_path)
            .arg("--leave-stopped")
            .output()
            .unwrap();

        // see if the job was really paused
        if status.status.code() != Some(0) {
            panic!(
                "Job failed to be paused: {:?}",
                String::from_utf8(status.stderr).unwrap()
            );
        }

        // continue the job
        kill(self.pid.as_ref().cloned().unwrap(), Signal::SIGCONT).unwrap();

        // folder dance:
        // 1. (*) dump into new folder
        // 2. rename old dump into a new folder
        // 3. rename new dump as the dump folder
        // 4. (*) delete old dump
        // this ensures that the amount of time that the proper dump path is
        // not a valid directory containing valid dump files is minimal
        //
        // we've done step 1 from this point.

        // determine whether the old dump exists as a directory
        let old_dump_meta =
            std::fs::metadata(&target_dump_path).map(|m| m.is_dir());
        let old_dump_exists = match old_dump_meta {
            Ok(true) => true,
            Ok(false) => {
                eprintln!(
                    "A file labelled {} has been deleted.",
                    target_dump_path.display()
                );
                std::fs::remove_file(&target_dump_path)?;
                false
            },
            Err(e) if e.kind() == ErrorKind::NotFound => false,
            Err(e) => {
                panic!(
                    "Failed to remove directory {}: {:?}",
                    target_dump_path.display(),
                    e
                )
            },
        };

        // rename the old dump folder away
        let old_dir_new_name = self.create_temp_dump_dir_name();
        if old_dump_exists {
            std::fs::rename(&target_dump_path, &old_dir_new_name)?;
        }

        // rename the new dump folder into
        std::fs::rename(&temp_path, &target_dump_path)?;

        // remove the old dump
        if old_dump_exists {
            std::fs::remove_dir_all(&old_dir_new_name)?;
        }

        Ok(())
    }

    pub fn restore(
        // TODO: this should just be a job ID. implement it when you've got
        // the database finished.
        file_path: &(impl AsRef<Path> + ?Sized),
    ) -> IoResult<ConversionJob> {
        // TODO: you should get the job ID from the database vvvvvvvvvvvvvvvvvvv
        let job_id = JobId(read_file_get_hash(file_path)?);

        // create a dummy Job object for now
        let dummy = ConversionJob {
            job_id,
            path: PathBuf::new(),

            // NOTE: do not rely on this. this is only a placeholder.
            pid: None,
        };
        // TODO: you should get the job ID from the database ^^^^^^^^^^^^^^^^^^^

        // create the file at which to write the PID into
        let mut pid_filename = dummy.job_path();
        pid_filename.push("pidfile.txt");

        // delete the pid file if it exists.
        // criu doesn't like it when it exists.
        core::mem::drop(std::fs::remove_file(&pid_filename));

        // resume the job
        Command::new("criu")
            .arg("restore")
            .arg("--images-dir")
            .arg(dummy.dump_path())
            .arg("--restore-detached")
            .arg("--pidfile")
            .arg(&pid_filename)
            .output()
            .unwrap();

        // read the contents of the PID file
        let pid_file =
            OpenOptions::new().read(true).open(&pid_filename).unwrap();
        let mut pid_str = String::new();
        BufReader::new(pid_file).read_line(&mut pid_str).unwrap();
        pid_str.pop();

        // TODO: raise a manual IoError upon read failure
        let pid = pid_str.parse::<i32>().unwrap();
        let pid = Pid::from_raw(pid);

        // delete the pid file if it exists.
        // criu doesn't like it when it exists.
        core::mem::drop(std::fs::remove_file(pid_filename));

        let job = ConversionJob {
            pid: Some(pid),
            path: <_ as AsRef<Path>>::as_ref(file_path).to_path_buf(),
            job_id,
        };
        Ok(job)
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
            file.seek(Start(65536)).unwrap();
        }
        else {
            file.seek(End(-65536)).unwrap();
        }

        let mut buffer = vec![0u8; 65536];
        let bytes_read = file.read(&mut buffer)?;

        buffer.truncate(bytes_read);

        hasher.write(&buffer);
    }

    Ok(hasher.finish())
}
