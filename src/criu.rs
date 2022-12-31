struct ConversionUnit {
    is_biggest: bool,
    file: PathBuf,
    process: (),
    database: Arc<Mutex<DatabaseConnection>>,
}

impl ConversionUnit {
    fn new(file: PathBuf, is_biggest: bool) -> ConversionUnit {
        unimplemented!()
    }

    fn try_read_from_file() {
    }

    fn initialize_command() {
    }

    fn run() {
        try_read_from_file();
    }

    fn on_finish(self) {

    }
}
