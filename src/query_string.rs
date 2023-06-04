use core::fmt::Write;
use std::{
    collections::HashMap,
    num::IntErrorKind,
};

#[derive(Debug, Clone)]
pub enum QueryStringErrorSource<'a> {
    AudioKey(&'a str, IntErrorKind),
    NoAudioKey,

    AudioFileSource(&'a str, IntErrorKind),
    AudioChannelSource(&'a str, IntErrorKind),
}

// taken directly from core::num::error.rs
fn iek_description(kind: IntErrorKind) -> &'static str {
    match kind {
        IntErrorKind::Empty => "cannot parse integer from empty string",
        IntErrorKind::InvalidDigit => "invalid digit found in string",
        IntErrorKind::PosOverflow => "number too large to fit in target type",
        IntErrorKind::NegOverflow => "number too small to fit in target type",
        IntErrorKind::Zero => "number would be zero for non-zero type",
        _ => "cannot parse integer: too general",
    }
}

impl<'a> QueryStringErrorSource<'a> {
    pub fn write_error_msg<W>(
        &self,
        writer: &mut W,
    ) -> Result<(), std::fmt::Error>
    where
        W: Write,
    {
        use QueryStringErrorSource::*;

        match self {
            AudioKey(s, kind) => {
                write!(
                    writer,
                    "Unable to parse audio key \"{}\" from query string: {}",
                    s,
                    iek_description(kind.clone())
                )
            },
            AudioFileSource(s, kind) => {
                write!(
                    writer,
                    "Unable to parse audio source file \"{}\" from query \
                     string: {}",
                    s,
                    iek_description(kind.clone())
                )
            },
            AudioChannelSource(s, kind) => {
                write!(
                    writer,
                    "Unable to parse audio source channel \"{}\" from query \
                     string: {}",
                    s,
                    iek_description(kind.clone())
                )
            },
            NoAudioKey => {
                write!(
                    writer,
                    "Unable to parse audio key from query string: does not \
                     exist"
                )
            },
        }
    }

    pub fn as_error_msg(&self) -> String {
        let mut retval = String::new();
        self.write_error_msg(&mut retval).unwrap();
        retval
    }
}

#[derive(Debug, Clone)]
pub(crate) struct QueryStringContents {
    audio_map: HashMap<usize, Vec<(usize, usize)>>,
}

pub(crate) fn get_requests<'a>(
    params: &'a str,
) -> Result<QueryStringContents, QueryStringErrorSource<'a>> {
    let mut audios = HashMap::new();

    for (key, value) in querystring::querify(params).into_iter() {
        let key_parts = key.split("_");

        match (key, value) {
            (a, v) if a.starts_with("audio_") => {
                let (a, o) = get_audio_query_parameter(a, v)?;
                audios.insert(a, o);
            },

            (key, _) => {
                eprintln!("Unrecognized query key `{}`", key);
            },
        }
    }

    Ok(QueryStringContents {
        audio_map: audios,
    })
}

fn get_audio_query_parameter<'a>(
    audio_key: &'a str,
    value: &'a str,
) -> Result<(usize, Vec<(usize, usize)>), QueryStringErrorSource<'a>> {
    use std::num::IntErrorKind as IEK;

    let target_index_str = audio_key
        .split('_')
        .skip(1)
        .take(1)
        .next()
        .ok_or_else(|| QueryStringErrorSource::NoAudioKey)?;
    let target_index = target_index_str.parse::<usize>().map_err(|e| {
        QueryStringErrorSource::AudioKey(target_index_str, e.kind().clone())
    })?;

    let mut ordering = vec![];

    for part in value.split(",") {
        let mut part_part = part.split(":");

        let source_file_str = part_part
            .next()
            .ok_or(QueryStringErrorSource::AudioFileSource("", IEK::Empty))?;
        let source_file = source_file_str.parse::<usize>().map_err(|e| {
            QueryStringErrorSource::AudioFileSource(source_file_str, e.kind().clone())
        })?;

        let source_channel_str = part_part.next().ok_or(
            QueryStringErrorSource::AudioChannelSource("", IEK::Empty),
        )?;
        let source_channel = source_channel_str.parse::<usize>().map_err(|e| {
            QueryStringErrorSource::AudioChannelSource(source_channel_str, e.kind().clone())
        })?;

        ordering.push((source_file, source_channel))
    }

    Ok((target_index, ordering))
}
