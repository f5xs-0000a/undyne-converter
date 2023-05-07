use std::{
    collections::VecDeque,
    ffi::OsStr,
    net::SocketAddr,
    path::{
        Path,
        PathBuf,
    },
};

use axum::{
    body::boxed,
    extract::Multipart,
    http::StatusCode,
    response::Response,
    routing::{
        on,
        MethodFilter,
    },
    Router,
};
use futures::TryStreamExt;
use serde::{
    Deserialize,
    Deserializer,
};
use tokio::{
    fs::OpenOptions,
    io::AsyncWriteExt,
    process::Command,
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
    where
        D: Deserializer<'de>,
    {
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

        let input_i = input_i.parse::<f64>().map_err(|err| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&input_i),
                &err.to_string().as_str(),
            )
        })?;

        let input_tp = input_tp.parse::<f64>().map_err(|err| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&input_tp),
                &err.to_string().as_str(),
            )
        })?;

        let input_lra = input_lra.parse::<f64>().map_err(|err| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&input_lra),
                &err.to_string().as_str(),
            )
        })?;

        let input_thresh = input_thresh.parse::<f64>().map_err(|err| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&input_thresh),
                &err.to_string().as_str(),
            )
        })?;

        Ok(AudioConstants {
            input_i,
            input_tp,
            input_lra,
            input_thresh,
        })
    }
}

/// Use FFmpeg to read the audio constants of a file
async fn determine_audio_constants(
    path: impl AsRef<OsStr>
) -> Vec<AudioConstants> {
    let mut constants = vec![];

    for channel_no in 0 .. {
        let audio_stats = Command::new("ffmpeg")
            .arg("-hide_banner")
            // read this specific file
            .arg("-i")
            .arg(&path)
            // ignore the video portion
            .arg("-vn")
            .arg("-map")
            .arg(&format!("0:a:{}", channel_no))
            // use the filter loudnorm to print the loudness constants in JSON
            .arg("-filter:a")
            .arg("loudnorm=print_format=json")
            // we're not writing anything so pipe the output into /dev/null with
            // null type
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

        if let Ok(consts) = serde_json::from_str(&object_string) {
            constants.push(consts);
        }
        else {
            break;
        }
    }

    constants
}

async fn convert_audio(
    constants: Vec<AudioConstants>,
    input_path: impl AsRef<OsStr>,
    target_i: f64,
) -> Vec<PathBuf> {
    let mut converted_audio_paths = vec![];

    for (idx, constant) in constants.iter().enumerate() {
        let filter_graph = format!(
            "loudnorm=linear=true:i={}:measured_I={}:measured_LRA={}:\
             measured_tp={}:measured_thresh={}",
            target_i,
            constant.input_i,
            constant.input_lra,
            constant.input_tp,
            constant.input_thresh
        );

        let path = format!("./audio_{}.opus", idx);

        let command = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-y")
            .arg("-i")
            .arg(&input_path)
            .arg("-vn")
            .arg("-map")
            .arg(&format!("0:a:{}", idx))
            // use the filter loudnorm to print the loudness constants in JSON
            .arg("-filter:a")
            .arg("loudnorm=print_format=json")
            // we're not writing anything so pipe the output into /dev/null with
            // null type
            .arg("-codec:a")
            .arg("libopus")
            .arg("-compression_level")
            .arg("10")
            .arg(&path)
            .kill_on_drop(true)
            .output()
            .await
            .unwrap();

        eprintln!("{}", String::from_utf8(command.stderr).unwrap());

        eprintln!("Created {}", path);
        converted_audio_paths.push(path.into());
    }

    converted_audio_paths
}

fn crf(width: usize, height: usize) -> usize {
    let width = width as f64;
    let height = height as f64;
    ((-0.0084 * (width * height).sqrt() + 40.22287) as isize).min(63).max(0) as usize
}

async fn determine_video_dimensions(path: impl AsRef<OsStr>) -> Option<(usize, usize)> {
    let command = Command::new("ffprobe")
        .arg("-hide_banner")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v")
        .arg("-show_entries")
        .arg("stream=width,height")
        .arg("-print_format")
        .arg("json")
        .arg(path)
        .output()
        .await
        .unwrap();

    #[derive(Deserialize)]
    struct Dimensions {
        width: usize,
        height: usize,
    }

    #[derive(Deserialize)]
    struct Entries {
        // programs:
        streams: Vec<Dimensions>,
    }

    let dimensions = String::from_utf8(command.stdout).unwrap();
    serde_json::from_str::<Entries>(&dimensions).ok().and_then(|e| e.streams.into_iter().next()).map(|dim| (dim.width, dim.height))
}

async fn do_job(path: impl AsRef<OsStr>) {
    eprintln!("Analyzing audio...");
    let audio_constants = determine_audio_constants(&path).await;
    eprintln!("{:?}", audio_constants);

    let converted_audios = convert_audio(audio_constants, &path, -18.).await;

    let first_pass_log = "./ffmpeg2pass-0.log";

    let (width, height) = determine_video_dimensions(&path).await.unwrap();
    let crf = crf(width, height);

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
        .arg("-passlogfile")
        .arg(&first_pass_log)
        .arg("-f")
        .arg("null")
        .arg("/dev/null")
        .output()
        .await
        .unwrap();

    eprintln!("Starting conversion...");
    let conversion = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-i")
        .arg(&path)
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

    // OPUS related options
    //.arg("-codec:a")
    //.arg("libopus")
    //.arg("-compression_level")
    //.arg("10")

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
