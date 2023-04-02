#[cfg(not(target_os = "linux"))]
compile_error!("This program can only be compiled for Linux");

use nix::unistd::Uid;
use std::process::Command;
use nix::unistd::geteuid;

fn main() {
    if !geteuid().is_root() {
        eprintln!("Application is not root. Have you tried running using sudo?");
    }
    
    spawn_and_pause();
    //resume_job();
}

fn resume_job() {
    let dump_dir = "test_dir/";
    
    // resume the job
    Command::new("criu")
        .arg("restore")
        .arg("--images-dir")
        .arg(dump_dir)
        .arg("--shell-job")
        .spawn()
        ;

    eprintln!("Job resumed!");

    std::thread::sleep(std::time::Duration::new(50000, 0));
}

fn get_calling_uid() -> Uid {
    match std::env::var("SUDO_UID") {
        Err(e) => panic!("Cannot extract environment variable SUDO_UID: {:?}", e),
        Ok(s) => match s.parse::<u32>() {
            Ok(uid) => Uid::from_raw(uid),
            Err(e) => panic!("Cannot convert UID {} into u32: {:?}", s, e),
        },
    }
}

// this is done
fn spawn_and_pause() {
    dbg!(get_calling_uid());

    return;

    let dump_dir = "test_dir/";

    eprintln!("Removing the contents of the test dump directory");
    core::mem::drop(std::fs::remove_dir_all(dump_dir));

    eprintln!("Creating the directory again...");
    std::fs::create_dir(dump_dir).unwrap();

    let python_spawned = Command::new("python")
        .arg("src/lel.py")
        .spawn()
        .unwrap();

    eprintln!("ID is {}", python_spawned.id());

    std::thread::sleep(std::time::Duration::new(5, 0));

    eprintln!("Pausing the job...");

    // pause the job
    let status = Command::new("criu")
        .arg("dump")
        .arg("--tree")
        .arg(&format!("{}", python_spawned.id()))
        .arg("--images-dir")
        .arg(dump_dir)
        .arg("--shell-job")
        .arg("--leave-stopped")
        .output()
        .unwrap();

    if status.status.code() != Some(0) {
        eprintln!("Job failed to be paused: {:?}", String::from_utf8(status.stderr));
        return;
    }

    eprintln!("Job paused!");

    let pid = nix::unistd::Pid::from_raw(python_spawned.id() as i32);

    nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGCONT);

    std::thread::sleep(std::time::Duration::new(50000, 0));
}
