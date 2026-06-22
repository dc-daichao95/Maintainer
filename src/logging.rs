use std::io::{self, Write};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::time::FormatTime;

pub struct TzTimer(chrono_tz::Tz);

impl TzTimer {
    pub fn new(tz: chrono_tz::Tz) -> Self {
        Self(tz)
    }
}

impl FormatTime for TzTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        let now = chrono::Utc::now().with_timezone(&self.0);
        write!(w, "{}", now.format("%Y-%m-%dT%H:%M:%S%.3f%:z"))
    }
}

pub struct IgnoreBrokenPipe<W>(pub W);

impl<'a, W> MakeWriter<'a> for IgnoreBrokenPipe<W>
where
    W: MakeWriter<'a> + 'a,
{
    type Writer = IgnoreBrokenPipeWriter<W::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        IgnoreBrokenPipeWriter(self.0.make_writer())
    }
}

pub struct IgnoreBrokenPipeWriter<W>(W);

impl<W: Write> Write for IgnoreBrokenPipeWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.0.write(buf) {
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(buf.len()),
            res => res,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.0.flush() {
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            res => res,
        }
    }
}
