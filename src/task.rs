use std::error::Error;
use std::fmt;
use std::str::Utf8Error;

use chrono::{Local, TimeZone};
use colored::*;

use crate::format::{card, format_duration, progress_bar, strip_colors};

pub fn read<T: std::io::Read, V, E, F>(
    stream: &mut T,
    convert: F,
    size: usize,
) -> Result<V, Box<dyn Error>>
where
    F: Fn(&[u8]) -> Result<V, E>,
    E: std::error::Error + 'static,
{
    let mut buf = vec![0; size];
    stream.read_exact(&mut buf)?;

    convert(&buf).map_err(|e| Box::new(e) as Box<dyn Error>)
}

pub fn read_i64<T: std::io::Read>(stream: &mut T) -> Result<i64, Box<dyn Error>> {
    read(
        stream,
        |b| Ok::<i64, CorruptError>(i64::from_be_bytes(b.try_into().map_err(|_| CorruptError)?)),
        size_of::<i64>(),
    )
}
pub fn read_usize<T: std::io::Read>(stream: &mut T) -> Result<usize, Box<dyn Error>> {
    read(
        stream,
        |b| {
            Ok::<usize, CorruptError>(usize::from_be_bytes(
                b.try_into().map_err(|_| CorruptError)?,
            ))
        },
        size_of::<usize>(),
    )
}
pub fn read_str<T: std::io::Read>(stream: &mut T, size: usize) -> Result<String, Box<dyn Error>> {
    read(
        stream,
        |b| Ok::<String, Utf8Error>(std::str::from_utf8(b).map(|x| x.to_owned())?),
        size,
    )
}

#[derive(Debug, Clone)]
struct CorruptError;
impl fmt::Display for CorruptError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Corrupt file")
    }
}
impl Error for CorruptError {}

#[derive(Debug, Clone)]
pub(crate) struct Task {
    id: i64,
    pub(crate) progress: i64,
    pub(crate) deadline: i64,
    pub(crate) estimated_time: i64,
    pub(crate) name: String,
    pub(crate) description: String,
}
impl Task {
    pub(crate) fn new() -> Self {
        Task {
            id: 0,
            progress: 0,
            deadline: 0,
            estimated_time: 0,
            name: String::new(),
            description: String::new(),
        }
    }

    pub(crate) fn with_id(id: i64) -> Self {
        let mut task = Task::new();
        task.id = id;
        task
    }

    pub(crate) fn with_details(
        id: i64,
        progress: i64,
        due: i64,
        estimated_time: i64,
        name: String,
        description: String,
    ) -> Self {
        Task {
            id,
            progress,
            deadline: due,
            name,
            description,
            estimated_time,
        }
    }

    pub(crate) fn id(&self) -> i64 {
        self.id
    }

    pub(crate) fn from<T: std::io::Read>(
        stream: &mut T,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut task = Task::with_details(
            read_i64(stream)?,
            read_i64(stream)?,
            read_i64(stream)?,
            read_i64(stream)?,
            String::new(),
            String::new(),
        );

        let name_len = read_usize(stream)?;
        task.name = read_str(stream, name_len)?;
        let desc_len = read_usize(stream)?;
        task.description = read_str(stream, desc_len)?;
        Ok(task)
    }

    pub(crate) fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::from(self.id.to_be_bytes());
        bytes.extend_from_slice(&self.progress.to_be_bytes());
        bytes.extend_from_slice(&self.deadline.to_be_bytes());
        bytes.extend_from_slice(&self.estimated_time.to_be_bytes());

        let name_bytes = self.name.as_bytes();
        bytes.extend_from_slice(&name_bytes.len().to_be_bytes());
        bytes.extend_from_slice(&name_bytes);

        let desc_bytes = self.description.as_bytes();
        bytes.extend_from_slice(&desc_bytes.len().to_be_bytes());
        bytes.extend_from_slice(&desc_bytes);

        bytes
    }

    pub(crate) fn get_completion(&self) -> f32 {
        if self.estimated_time == 0 {
            return 1.0;
        }
        (self.progress as f32) / (self.estimated_time as f32)
    }

    pub(crate) fn get_time_left(&self) -> i64 {
        self.deadline - Local::now().timestamp()
    }

    pub(crate) fn format_due(&self) -> String {
        Local
            .timestamp_opt(self.deadline.into(), 0)
            .unwrap()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    }

    pub(crate) fn to_string(&self) -> String {
        let tl = self.get_time_left();
        let mut s = strip_colors(&format_duration(tl));

        s = if tl >= 7 * 24 * 60 * 60 {
            s.blue()
        } else if tl >= 2 * 24 * 60 * 60 {
            s.green()
        } else if tl >= 24 * 60 * 60 {
            s.yellow()
        } else if tl >= 5 * 60 * 60 {
            s.red()
        } else {
            s.bright_red()
        }
        .to_string();

        let strings: Vec<(String, String)> = vec![
            (
                "Name:".truecolor(128, 128, 128).bold().to_string(),
                self.name.bold().to_string(),
            ),
            (
                "Description:".truecolor(128, 128, 128).bold().to_string(),
                self.description.italic().to_string(),
            ),
            (
                "Deadline:".truecolor(128, 128, 128).bold().to_string(),
                self.format_due().truecolor(255, 140, 0).to_string(),
            ),
            ("Time left:".truecolor(128, 128, 128).bold().to_string(), s),
            (
                "Time to complete:"
                    .truecolor(128, 128, 128)
                    .bold()
                    .to_string(),
                format_duration(self.estimated_time - self.progress.min(self.estimated_time))
                    .to_string(),
            ),
            (
                "Progress:".truecolor(128, 128, 128).bold().to_string(),
                progress_bar(self.get_completion()).to_string(),
            ),
            (
                "Id:".truecolor(128, 128, 128).bold().to_string(),
                self.id.to_string().cyan().to_string(),
            ),
        ];

        format!("{}", card(strings)).to_owned()
    }
}
impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}
