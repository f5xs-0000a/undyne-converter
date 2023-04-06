// this trait import automatically assumes that it will work for Unix-based
// operating systems and not Windows
use std::{
    fs::OpenOptions,
    io::{
        BufRead as _,
        BufReader,
    },
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

fn main() {
    if !geteuid().is_root() {
        eprintln!(
            "Application is not root. Have you tried running using sudo?"
        );
    }

    /*
    {
        let converter = ConversionJob::new(
            PathBuf::from("./src/test_files/Coffee Run.webm"),
            get_sudo_invoker(),
        );

        converter.dump();
    }
    */

    {
        let converter = ConversionJob::restore(&PathBuf::from("./target_dump"));

        converter.dump();
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
}

// NOTE: we're just doing a read job. we're still not doing a conversion job.

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
        // create the temporary folder
        // TODO: you have to create a folder that does not exist yet, and
        // said process must be locked behind a mutex. only one of it (making
        // the folder) can happen at a time.
        let temp_folder = PathBuf::from("./temp-0123456789abcdef/");
        let target_folder = PathBuf::from("./target_dump");
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
        let target_folder = PathBuf::from("./target_dump");
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
