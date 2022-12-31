

#[derive(Args)]
struct RunApp {
    database: PathBuf,
    threads: Option<usize>,
    state_path: PathBuf,
}

impl RunApp {
    fn check(&self) -> bool {
        if !check_packages() {
            false
        }

        eprint!("Checking if you are root... ");
        if get_shell_id("UID") != 0 {
            eprintln!(
                "You are not root. Please consider running this program as sudo."
            );
            return false;
        }
        eprintln!("You are root.");

        let invoker = get_shell_id("SUDO_UID");
        eprintln!("Invoker (you) is user #{}.", invoker);

        eprintln!("Checking if the database exists...");
        if !self.database.exists() {
            eprintln!("Database file does not exist yet. Creating one...");

            unimplemented!();
        }

        else if self.database.is_file() {
            eprintln!("Database path is not a file. Aborting.");
            return false;
        }

        // if the file isn't a database file
        else if unimplemented!() {
            eprintln!("Database path is not a valid sqlite database. Aborting.");
            return false;
        }

        eprintln("Checking if the dump file exists...");
    }

    fn get_threads(&self) -> usize {
        self.threads.clone().unwrap_or(1).max(1)
    }

    fn run(&self) {

    }
}

fn check_packages() -> bool {
    let required_packages = ["sh", "criu", "ffmpeg", "sudo"];
    for package in required_packages.iter() {
        eprint!("Checking if `{}` exists...", package);
        match Command::new(package).output() {
            Err(_) => {
                eprintln!(
                    " `{}` does not exist. Install it first before proceeding",
                    package
                );
                return false;
            },
            Ok(_) => eprintln!(" `{}` exists.", package),
        }
    }

    true
}

fn get_shell_id(var: &str) -> u32 {
    let stdout = Command::new("sh")
        .arg("-c")
        .arg(&format!("echo ${}", var))
        .output()
        .unwrap()
        .stdout;

    let len = stdout.len();

    let str_id = match String::from_utf8(stdout[.. len - 1].to_vec()) {
        Ok(id) => id,
        _ => panic!("`sh` returned a non-utf8 output."),
    };

    match str_id.parse() {
        Ok(id) => id,
        _ => panic!("Unable to parse output `{}`", str_id),
    }
}
