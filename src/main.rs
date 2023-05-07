use std::{
    net::SocketAddr,
};
use std::ffi::OsStr;
use std::collections::VecDeque;

use serde::Deserialize;
use serde::Deserializer;
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

#[derive(Debug, Clone)]
struct AudioConstants {
    input_i: f64,
	input_tp: f64,
	input_lra: f64,
	input_thresh: f64,
	//output_i:
	//output_tp:
	//output_lra:
	//output_thresh:
	//normalization_type:
	//target_offset:
}

impl<'de> Deserialize<'de> for AudioConstants {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> {
        #[derive(Deserialize)]
        struct StringAudioConstants {
            input_i: String,
            input_tp: String,
            input_lra: String,
            input_thresh: String,
        }

        let StringAudioConstants {
            input_i,
            input_tp,
            input_lra,
            input_thresh,
        } = StringAudioConstants::deserialize(deserializer)?;

        let input_i = input_i
            .parse::<f64>()
            .map_err(|err| serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&input_i),
                &err.to_string().as_str()
            ))?;

        let input_tp = input_tp
            .parse::<f64>()
            .map_err(|err| serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&input_tp),
                &err.to_string().as_str()
            ))?;

        let input_lra = input_lra
            .parse::<f64>()
            .map_err(|err| serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&input_lra),
                &err.to_string().as_str()
            ))?;

        let input_thresh = input_thresh
            .parse::<f64>()
            .map_err(|err| serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&input_thresh),
                &err.to_string().as_str()
            ))?;

        Ok(AudioConstants {
            input_i,
            input_tp,
            input_lra,
            input_thresh,
        })
    }
}

/// Use FFmpeg to read the audio constants of a file
async fn determine_audio_constants(path: impl AsRef<OsStr>) -> Option<AudioConstants> {
    // TODO: what if there is no audio? what if there is more than one audio?

    let audio_stats = Command::new("ffmpeg")
        .arg("-hide_banner")
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

    let mut line_ring = VecDeque::with_capacity(12);
    let stderr = String::from_utf8(audio_stats.stderr).unwrap();
    let input_lines = stderr.lines();

    // get the last 12 lines
    for line in input_lines {
        while line_ring.capacity() <= line_ring.len() {
            line_ring.pop_front();
        }

        line_ring.push_back(line);
    }

    // form the string
    let mut object_string = String::new();
    for line in line_ring.into_iter() {
        object_string += line;
    }

    serde_json::from_str(&object_string).ok()
}

async fn do_job(path: impl AsRef<OsStr>) {
    eprintln!("Analyzing audio...");
    let audio_constants = determine_audio_constants(&path).await;
    eprintln!("{:?}", audio_constants);

    panic!();
 
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
        // OPUS related options
        .arg("-codec:a")
        .arg("libopus")
        .arg("-compression_level")
        .arg("10")
        // General video options
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
