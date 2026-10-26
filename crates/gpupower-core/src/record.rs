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
    use crate::{amd_raphael, gtx1050_error, rtx3090_primary, AdapterSnapshot, Sampler, VendorSource};

    struct StringWriter(String);
    impl SampleWriter for StringWriter {
        fn write_chunk(&mut self, text: &str) -> Result<(), WriteError> {
            self.0.push_str(text);
            Ok(())
        }
    }

    struct FailingWriter(WriteError);
    impl SampleWriter for FailingWriter {
        fn write_chunk(&mut self, _: &str) -> Result<(), WriteError> {
            Err(self.0.clone())
        }
    }

    struct FailingOnSecondWriter(WriteError, usize);
    impl SampleWriter for FailingOnSecondWriter {
        fn write_chunk(&mut self, _: &str) -> Result<(), WriteError> {
            self.1 += 1;
            if self.1 == 2 {
                Err(self.0.clone())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn test_recorder_csv() {
        let mut recorder = Recorder::start(RecordFormat::Csv, StringWriter(String::new()));

        let rtx = rtx3090_primary();
        let mut gtx = gtx1050_error();
        gtx.error = Some("device error".to_string());
        let amd = amd_raphael();

        let step = recorder.push("t", &[rtx.clone(), gtx.clone(), amd.clone()]);
        assert_eq!(step, RecordStep::Wrote);

        let finished = recorder.finish_document();
        assert_eq!(finished, Ok(String::new()));

        let expected = format!(
            "{}\n{}\n{}\n{}\n",
            csv_header(),
            csv_row("t", &rtx),
            csv_row("t", &gtx),
            csv_row("t", &amd)
        );
        assert_eq!(recorder.writer.0, expected);
    }

    #[test]
    fn test_recorder_jsonl() {
        let mut recorder = Recorder::start(RecordFormat::Jsonl, StringWriter(String::new()));

        let rtx = rtx3090_primary();
        let mut gtx = gtx1050_error();
        gtx.error = Some("device error".to_string());
        let amd = amd_raphael();

        let step = recorder.push("t", &[rtx.clone(), gtx.clone(), amd.clone()]);
        assert_eq!(step, RecordStep::Wrote);

        let finished = recorder.finish_document();
        assert_eq!(finished, Ok(String::new()));

        let expected = format!(
            "{}\n{}\n{}\n",
            jsonl_line("t", &rtx),
            jsonl_line("t", &gtx),
            jsonl_line("t", &amd)
        );
        assert_eq!(recorder.writer.0, expected);
    }

    #[test]
    fn test_recorder_xml() {
        let mut recorder = Recorder::start(RecordFormat::Xml, StringWriter(String::new()));

        let rtx = rtx3090_primary();
        let mut gtx = gtx1050_error();
        gtx.error = Some("device error".to_string());
        let amd = amd_raphael();

        let fixtures = [rtx, gtx, amd];
        let step = recorder.push("t", &fixtures);
        assert_eq!(step, RecordStep::Wrote);

        let finished = recorder.finish_document();
        assert_eq!(finished, Ok("</samples>".to_string()));

        let expected = format!(
            "<samples>{}</samples>",
            xml_sample_chunk("t", &fixtures)
        );
        assert_eq!(recorder.writer.0, expected);
    }

    #[test]
    fn test_recorder_empty() {
        let mut recorder_csv = Recorder::start(RecordFormat::Csv, StringWriter(String::new()));
        assert_eq!(recorder_csv.finish_document(), Ok(String::new()));
        assert_eq!(recorder_csv.writer.0, format!("{}\n", csv_header()));

        let mut recorder_jsonl = Recorder::start(RecordFormat::Jsonl, StringWriter(String::new()));
        assert_eq!(recorder_jsonl.finish_document(), Ok(String::new()));
        assert_eq!(recorder_jsonl.writer.0, "");

        let mut recorder_xml = Recorder::start(RecordFormat::Xml, StringWriter(String::new()));
        assert_eq!(recorder_xml.finish_document(), Ok("</samples>".to_string()));
        assert_eq!(recorder_xml.writer.0, "<samples></samples>");
    }

    #[test]
    fn test_recorder_write_error() {
        // Failing on start for Csv
        let mut recorder = Recorder::start(RecordFormat::Csv, FailingWriter(WriteError::DiskFull));
        assert_eq!(recorder.stopped, Some(DISK_FULL_REASON.to_string()));
        let step = recorder.push("t", &[]);
        assert_eq!(step, RecordStep::Stopped { reason: DISK_FULL_REASON.to_string() });
        assert_eq!(recorder.finish_document(), Ok(String::new())); // Returns Ok("") since it's stopped and not Xml

        // Failing on push for Jsonl (start succeeds because it doesn't write)
        let mut recorder = Recorder::start(RecordFormat::Jsonl, FailingWriter(WriteError::Unavailable("broken".into())));
        assert_eq!(recorder.stopped, None);
        let step = recorder.push("t", &[rtx3090_primary()]);
        assert_eq!(step, RecordStep::Stopped { reason: UNAVAILABLE_REASON.to_string() });
        assert_eq!(recorder.stopped, Some(UNAVAILABLE_REASON.to_string()));

        // Failing on finish for Xml
        let mut recorder = Recorder::start(RecordFormat::Xml, FailingOnSecondWriter(WriteError::DiskFull, 0));
        assert_eq!(recorder.finish_document(), Err(WriteError::DiskFull));
        assert_eq!(recorder.stopped, Some(DISK_FULL_REASON.to_string()));
    }

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
    fn test_recording_file_names() {
        assert_eq!(
            recording_file_name("20241026-123456", RecordFormat::Csv),
            "GpuPower-20241026-123456.csv"
        );
        assert_eq!(
            recording_file_name("20241026-123456", RecordFormat::Xml),
            "GpuPower-20241026-123456.xml"
        );
        assert_eq!(
            recording_file_name("20241026-123456", RecordFormat::Jsonl),
            "GpuPower-20241026-123456.jsonl"
        );
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
