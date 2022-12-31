use crate::utilities::DB;

#[derive(Args)]
struct AddApp {
    // TODO: this should default to false
    multi: bool,

    database: PathBuf,

    paths: Vec<PathBuf>,
}

// TODO: add a choice between a single video and a series multi video
// multi video will most likely be an optional flag due to how rare it is

impl AddApp {
    async fn add(self) {
        let connection = match crate::utilities::check_and_read_database_file(
            self.database
        ) {
            None => return,
            Some(x) => x,
        };

        if self.multi {
            self.multi_add(connection).await;
        }

        else {
            self.single_add(connection).await;
        }
    }

    async fn single_add(self, connection: Arc<Mutex<DB_CONN>>) {
        let mut connection_lock = connection.lock().unwrap();
        let mut transaction = conn
            .begin()
            .await
            .expect("Cannot start transaction");

        let mut hashes_iter = self
            .paths
            .iter()
            // add the hash and the canonicalized form of the path
            .map(|p| {
                let canon_path = p.canonicalize()?;
                let (hash, size) = p.get_file_hash_and_size(p)?;
                (canon_path, hash, size);
            });

        for (path, hash, size) in hashes.into_iter() {
            let query = sqlx::query!("
                    INSERT INTO hashes VALUES ($1, $2);
                    INSERT INTO files VALUES ($3, 0, $4);
                ",
                hash,
                size,
                hash,
                path,
            );

            query.execute(&mut transaction)
                .await
                .expect("Cannot insert values");
        }

        transaction.commit().await;
    }

    fn multi_add(self, connection: <DB as Database>) {
        assert!(paths.len() > 1);

        if paths.len() == 1 {
            eprintln!("Used the `multi` flag when there's only one file.");
            self.single_add(connection);
        }

        assert!(paths.len() >= 2);

        let mut connection_lock = connection.lock().unwrap();
        let mut transaction = conn
            .begin()
            .await
            .expect("Cannot start transaction");

        let mut hashes_iter = self
            .paths
            .iter()
            // add the hash and the canonicalized form of the path
            .map(|p| {
                let canon_path = p.canonicalize()?;
                let (hash, size) = p.get_file_hash_and_size(p)?;
                (canon_path, hash, size);
            });

        for (path, hash, size) in hashes.into_iter() {
            let query = sqlx::query!("
                    INSERT INTO hashes VALUES ($1, $2);
                    INSERT INTO files VALUES ($3, 0, $4);
                ",
                hash,
                size,
                hash,
                path,
            );

            query.execute(&mut transaction)
                .await
                .expect("Cannot insert values");
        }

        transaction.commit().await;
    }
}

fn get_file_hash_and_size(
    &self,
    paths: impl Iterator<Item = &Path>
) -> Result<(String, usize), std::io::Error> {
    let mut context = Context::new();
    let mut size = 0;

    for path in paths {
        let file = OpenOptions::new()
            .read(true)
            .open(path)?;

        for byte in file {
            // the most dogshit implementation there probably is
            context.consume([byte]);
            size += 1;
        }
    }

    Ok(format!("{:x}", context.compute()))
}
