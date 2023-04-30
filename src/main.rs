use std::{
    net::SocketAddr,
};
use std::ffi::OsStr;

use tokio::process::Command;
use std::path::Path;
use axum::{
    body::{
        boxed,
    },
    extract::{
        Multipart,
    },
    http::StatusCode,
    response::Response,
    routing::{
        on,
        MethodFilter,
    },
    Router,
};
use futures::{
    TryStreamExt,
};
use tokio::{
    fs::{
        OpenOptions,
    },
    io::{
        AsyncWriteExt,
    },
};

// we're getting ahead of ourselves in here.
/*
type JobId = u64;

enum JobPhase {
    NoAudio,

    Finished,
}

struct Job {
    path: PathBuf,
}

impl Job {
    pub async fn new(path: PathBuf) -> Job {
        Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-vn")
            .arg("-i")
            .arg(&path)
            .arg("-f")
            .arg("null")
            .arg("/dev/null")
    }
}

pub struct ConversionState {
    in_progress: HashMap<u64, Job>,
}

impl ConversionState {
    fn advance(&mut self)
}
*/

async fn do_job(path: impl AsRef<OsStr>) {
    eprintln!("Analyzing audio...");

    // read the audio stats
    let audio_stats = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("0")
        .arg("-i")
        .arg(&path)
        .arg("-vn")
        .arg("-filter:a")
        .arg("loudnorm=print_format=json")
        .arg("-f")
        .arg("null")
        .arg("/dev/null")
        .kill_on_drop(true)
        .output()
        .await
        .unwrap();

    let mut audio_stats = String::from_utf8(audio_stats.stderr);
    eprintln!("{}", audio_stats.unwrap());

    eprintln!("Performing first pass...");
    
    // generate first pass log
    let first_pass_log = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-i")
        .arg(&path)
        .arg("-codec:v")
        .arg("libaom-av1")
        .arg("-an")
        .arg("-pass")
        .arg("1")
        // TODO: use the -passlogfile argument
        .arg("-f")
        .arg("null")
        .arg("/dev/null")
        .output()
        .await
        .unwrap();

    let first_pass_log = "./ffmpeg2pass-0.log";

    let crf = 35; // TODO: unimplemented

    eprintln!("Starting conversion...");
    let conversion = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-i")
        .arg(&path)
        // OPUS related audio
        .arg("-codec:a")
        .arg("libopus")
        .arg("-compression_level")
        .arg("10")
        // General Video Options
        .arg("-codec:v")
        .arg("libaom-av1")
        .arg("-crf")
        .arg(&format!("{}", crf))
        .arg("-pass")
        .arg("2")
        .arg("-threads")
        .arg("1")
        .arg("-cpu-used")
        .arg("0")
        // AOM-AV1 specific flags start
        .arg("-auto-alt-ref")
        .arg("1")
        .arg("-arnr-max-frames")
        .arg("7")
        .arg("-arnr-strength")
        .arg("4")
        .arg("-tune")
        .arg("0")
        .arg("-lag-in-frames")
        .arg("35")
        .arg("-tile-columns")
        .arg("0")
        .arg("-row-mt")
        .arg("1")
        .arg("output.webm")
        // AOM-AV1 specific flags end
        .output()
        .await
        .unwrap();

    let mut conversion = String::from_utf8(conversion.stderr);
    eprintln!("{}", conversion.unwrap());

}

#[tokio::main]
async fn main() {
    do_job("./src/test_files/Coffee Run.webm").await;

    /*
    let router = Router::new()
        .route("/upload", on(MethodFilter::POST, upload));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap();
    */
}

pub async fn upload(mut multipart: Multipart) -> Result<Response, StatusCode> {
    while let Some(mut field) = match multipart.next_field().await {
        Err(_e) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        Ok(None) => None,
        Ok(Some(field)) => Some(field),
    } {
        let filename = match field.file_name() {
            None => return Err(StatusCode::INTERNAL_SERVER_ERROR),
            Some(fname) => fname.to_owned(),
        };
        let mut file = match OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filename)
            .await
        {
            Err(_e) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
            Ok(f) => dbg!(f),
        };

        {
            while let Some(bytes) = field.try_next().await.unwrap() {
                file.write(&bytes).await.unwrap();
            }
            file.flush().await.unwrap();

            // TODO: this should be an async block to receive a file. once a
            // file has been downloaded, a process should check the file if it
            // is a valid file
            
            // one way to do this is to use two processors in a join, one to
            // process sequential downloads and the other to check if the file
            // downloaded is a valid file
        }

        eprintln!("written {}", filename);
    }

    Ok(Response::builder()
        .status(StatusCode::CREATED)
        .body(boxed("Ok".to_owned()))
        .unwrap())
}
