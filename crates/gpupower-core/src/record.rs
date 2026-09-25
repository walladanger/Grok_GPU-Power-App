use crate::export::{csv_header, csv_row, jsonl_line, XmlExport};
use crate::snapshot::AdapterSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordFormat {
    Csv,
    Xml,
    Jsonl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteError {
    DiskFull,
    Unavailable(String),
}

pub trait SampleWriter {
    fn write_chunk(&mut self, text: &str) -> Result<(), WriteError>;
}

pub struct Recorder<W> {
    format: RecordFormat,
    writer: W,
    stopped: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordStep {
    Wrote,
    Stopped { reason: String },
}

const DISK_FULL_REASON: &str =
    "Recording stopped because the disk is full. Monitoring is still running.";
const UNAVAILABLE_REASON: &str =
    "Recording stopped because the file is unavailable. Monitoring is still running.";

impl<W: SampleWriter> Recorder<W> {
    pub fn start(format: RecordFormat, writer: W) -> Self {
        let mut recorder = Self {
            format,
            writer,
            stopped: None,
        };
        let opening = match format {
            RecordFormat::Csv => Some(with_newline(csv_header())),
            // The sample chunks below are the interior of this root.
            RecordFormat::Xml => Some("<samples>".to_string()),
            RecordFormat::Jsonl => None,
        };
        if let Some(opening) = opening {
            if let Err(error) = recorder.writer.write_chunk(&opening) {
                recorder.stopped = Some(stop_reason(&error));
            }
        }
        recorder
    }

    pub fn push(&mut self, timestamp_iso: &str, cards: &[AdapterSnapshot]) -> RecordStep {
        if let Some(reason) = &self.stopped {
            return RecordStep::Stopped {
                reason: reason.clone(),
            };
        }
        match self.format {
            RecordFormat::Csv => {
                let rows = cards
                    .iter()
                    .map(|card| with_newline(csv_row(timestamp_iso, card)));
                self.write_each(rows)
            }
            RecordFormat::Jsonl => {
                let lines = cards
                    .iter()
                    .map(|card| with_newline(jsonl_line(timestamp_iso, card)));
                self.write_each(lines)
            }
            RecordFormat::Xml => {
                self.write_each(std::iter::once(xml_sample_chunk(timestamp_iso, cards)))
            }
        }
    }

    pub fn finish_document(&mut self) -> Result<String, WriteError> {
        if self.format != RecordFormat::Xml || self.stopped.is_some() {
            return Ok(String::new());
        }
        let closing = "</samples>";
        if let Err(error) = self.writer.write_chunk(closing) {
            self.stopped = Some(stop_reason(&error));
            return Err(error);
        }
        Ok(closing.to_string())
    }

    fn write_each(&mut self, records: impl Iterator<Item = String>) -> RecordStep {
        for record in records {
            if let Err(error) = self.writer.write_chunk(&record) {
                let reason = stop_reason(&error);
                self.stopped = Some(reason.clone());
                return RecordStep::Stopped { reason };
            }
        }
        RecordStep::Wrote
    }
}

pub fn recording_file_name(stamp: &str, format: RecordFormat) -> String {
    let extension = match format {
        RecordFormat::Csv => "csv",
        RecordFormat::Xml => "xml",
        RecordFormat::Jsonl => "jsonl",
    };
    format!("GpuPower-{stamp}.{extension}")
}

// `csv_row` and `jsonl_line` do not end in a newline. One chunk is one finished record.
fn with_newline(mut text: String) -> String {
    text.push('\n');
    text
}

fn xml_sample_chunk(timestamp_iso: &str, cards: &[AdapterSnapshot]) -> String {
    let mut xml = XmlExport::new();
    xml.push(timestamp_iso, cards);
    let document = xml.finish();
    document
        .strip_prefix("<samples>")
        .and_then(|body| body.strip_suffix("</samples>"))
        .expect("XmlExport wraps one sample in <samples>")
        .to_string()
}

fn stop_reason(error: &WriteError) -> String {
    match error {
        WriteError::DiskFull => DISK_FULL_REASON.to_string(),
        WriteError::Unavailable(_) => UNAVAILABLE_REASON.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rtx3090_primary, AdapterSnapshot, Sampler, VendorSource};

    #[test]
    fn disk_full_stops_recording_and_keeps_sample() {
        struct Full;
        impl SampleWriter for Full {
            fn write_chunk(&mut self, _: &str) -> Result<(), WriteError> {
                Err(WriteError::DiskFull)
            }
        }
        struct OneCard;
        impl VendorSource for OneCard {
            fn sample(&mut self) -> Result<Vec<AdapterSnapshot>, String> {
                Ok(vec![rtx3090_primary()])
            }
        }
        struct NoneAmd;
        impl VendorSource for NoneAmd {
            fn sample(&mut self) -> Result<Vec<AdapterSnapshot>, String> {
                Ok(vec![])
            }
        }
        let mut recorder = Recorder::start(RecordFormat::Csv, Full);
        let step = recorder.push("t", &[rtx3090_primary()]);
        assert!(matches!(step, RecordStep::Stopped { reason } if reason.contains("disk is full")));
        let tick = Sampler {
            nvidia: OneCard,
            amd: NoneAmd,
            windows: vec![],
        }
        .tick();
        assert_eq!(tick.cards.len(), 1);
        assert!(tick.nvidia_error.is_none());
    }

    #[test]
    fn recording_name_uses_the_spec_stamp() {
        assert_eq!(
            recording_file_name("20260924-212908", RecordFormat::Csv),
            "GpuPower-20260924-212908.csv"
        );
        assert_eq!(
            recording_file_name("20260924-212908", RecordFormat::Xml),
            "GpuPower-20260924-212908.xml"
        );
        assert_eq!(
            recording_file_name("20260924-212908", RecordFormat::Jsonl),
            "GpuPower-20260924-212908.jsonl"
        );
    }
}
