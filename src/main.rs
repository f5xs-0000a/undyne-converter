use std::{
    collections::VecDeque,
    ffi::OsStr,
    net::SocketAddr,
    path::{
        Path,
        PathBuf,
    },
};
use std::sync::Arc;
use core::mem::drop;

use tokio::join;
use tokio::select;
use core::future::Future;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
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

#[derive(Debug, Clone)]
pub struct AudioConstants {
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

    for unbounded_channel_no in 0 .. {
        let audio_stats = Command::new("ffmpeg")
            .arg("-hide_banner")
            // read this specific file
            .arg("-i")
            .arg(&path)
            // ignore the video portion
            .arg("-vn")
            .arg("-map")
            .arg(&format!("0:a:{}", unbounded_channel_no))
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

async fn convert_audio_tracks(
    constants: &[AudioConstants],
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
            .arg(&filter_graph)
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

pub enum JobToOverseerMessage {
    // finished progresses
    //AudioFirstPassFinished,
    AudioSecondPassFinished,
    VideoFirstPassFinished,
    VideoSecondPassFinished,

    AudioConstantsDetermined(Arc<[AudioConstants]>), // aka AudioFirstPassFinished
    VideoDimensionsDetermined(usize, usize),
    VideoCrfDetermined(usize),
    
    VideoSecondPassProgress(PathBuf),
}


#[derive(Copy, Clone)]
pub enum AudioVideoStatus {
    FirstPass,
    SecondPass,
    Finished,
}

#[derive(Clone)]
pub struct JobStatus {
    audio: AudioVideoStatus,
    video: AudioVideoStatus,

    audio_constants: Option<Arc<[AudioConstants]>>,
    dimensions: Option<(usize, usize)>,
    crf: Option<usize>,

    video_conversion_log_path: Option<PathBuf>,
}

impl JobStatus {
    fn new() -> JobStatus {
        JobStatus {
            audio: AudioVideoStatus::FirstPass,
            video: AudioVideoStatus::FirstPass,

            audio_constants: None,
            dimensions: None,
            crf: None,

            video_conversion_log_path: None,
        }
    }

    fn process_update(&mut self, update: JobToOverseerMessage) {
        use JobToOverseerMessage::*;

        // TODO and FIXME: fix invalid state updates
        match update {
            AudioSecondPassFinished => self.audio = AudioVideoStatus::Finished,
            VideoFirstPassFinished => self.video = AudioVideoStatus::SecondPass,
            VideoSecondPassFinished => self.video = AudioVideoStatus::Finished,

            AudioConstantsDetermined(audio_constants) => self.audio_constants = Some(audio_constants),
            VideoDimensionsDetermined(width, height) => self.dimensions = Some((width, height)),
            VideoCrfDetermined(crf) => self.crf = Some(crf),
            
            VideoSecondPassProgress(path) => self.video_conversion_log_path = Some(path),
        }
    }
}

async fn convert_audio(path: impl AsRef<OsStr>, sender: UnboundedSender<JobToOverseerMessage>) -> Vec<PathBuf> {
    let audio_constants = determine_audio_constants(&path).await;
    let audio_constants: Arc<[AudioConstants]> = Arc::from(audio_constants);
    drop(sender.send(JobToOverseerMessage::AudioConstantsDetermined(audio_constants.clone())));

    let converted_audios = convert_audio_tracks(&*audio_constants, &path, -18.).await;
    drop(sender.send(JobToOverseerMessage::AudioSecondPassFinished));

    converted_audios
}

async fn convert_video(path: impl AsRef<OsStr>, sender: UnboundedSender<JobToOverseerMessage>) -> PathBuf {
    let crf_determine_future = async {
        let (width, height) = determine_video_dimensions(&path).await.unwrap();
        drop(sender.send(JobToOverseerMessage::VideoDimensionsDetermined(width, height)));

        let video_crf = crf(width, height);
        drop(sender.send(JobToOverseerMessage::VideoCrfDetermined(video_crf)));

        video_crf
    };

    // TODO: temporary name
    let first_pass_log = "./ffmpeg2pass-0.log";

    let first_pass_future = async {
        Command::new("ffmpeg")
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
        drop(sender.send(JobToOverseerMessage::VideoFirstPassFinished));
    };

    let (crf, _) = join!(
        crf_determine_future,
        first_pass_future,
    );

    eprintln!("Starting conversion...");
    // TODO: add message here that conversion video conversion has started
    // and send the supposed log file
    Command::new("ffmpeg")
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
    drop(sender.send(JobToOverseerMessage::VideoSecondPassFinished));

    "output.webm".into()
}

async fn merge_media(audio: Vec<PathBuf>, video: PathBuf, sender: UnboundedSender<JobToOverseerMessage>) -> PathBuf {
    unimplemented!()
}

pub struct RequestForJobStatus;

/// The future that is returned by `run_job`.
async fn actually_run_job(
    path: impl AsRef<OsStr>,
    status_sender: UnboundedSender<JobStatus>,
    mut request_receiver: UnboundedReceiver<RequestForJobStatus>,
) -> PathBuf {
    let (update_sender, mut update_receiver) = unbounded_channel();

    let main_job_future = async {
        let (audio_files, video_file) = join!(
            convert_audio(&path, update_sender.clone()),
            convert_video(&path, update_sender.clone()),
        );

        let merged = merge_media(audio_files, video_file, update_sender).await;

        // TODO: delete temporary files
        merged
    };

    let message_processor_future = async {
        // TODO: to prevent DDOS attacks, use Arc<RwLock<_>>
        let mut state = JobStatus::new();

        loop {
            select! {
                biased;

                // receive updates from our job
                message = update_receiver.recv() => {
                    let message = match message {
                        Some(m) => m,
                        None => break,
                    };

                    state.process_update(message);
                },

                // receive request for updates from caller
                _ = request_receiver.recv() => {
                    drop(status_sender.send(state.clone()));
                },
            }
        }
    };

    select! {
        retval = main_job_future => return retval,
        _ = message_processor_future => unreachable!(),
    }
}

/// Runs a job and returns both the future and the receiver for its messages.
///
/// In order to communicate between the caller thread and this function to
/// request and send status updates, a channel must be made for (1) request for
/// status, and (2) the status themselves.
///
/// (1) For the request for status, the receiver is held by this function and
/// the sender is sent back to the caller
/// (2) For the status, the receiver is sent to the caller and the sender is
/// held by this function
pub fn run_job(path: PathBuf) -> (impl Future<Output = PathBuf>, UnboundedReceiver<JobStatus>, UnboundedSender<RequestForJobStatus>) {
    let (status_sender, status_receiver) = unbounded_channel();
    let (request_sender, request_receiver) = unbounded_channel();

    (
        actually_run_job(path, status_sender, request_receiver),
        status_receiver,
        request_sender
    )
}

#[tokio::main]
async fn main() {
    //do_job("./src/test_files/Coffee Run.webm").await;

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
