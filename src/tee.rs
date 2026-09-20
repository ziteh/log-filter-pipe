use std::io::{Read, Write};

pub struct TeeReader<R, W> {
    inner: R,
    sink: Option<W>,
}

impl<R: Read, W: Write> TeeReader<R, W> {
    pub fn new(inner: R, sink: W) -> Self {
        Self {
            inner,
            sink: Some(sink),
        }
    }
}

impl<R: Read, W: Write> Read for TeeReader<R, W> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        if let Some(sink) = self.sink.as_mut()
            && sink.write_all(&buf[..n]).is_err()
        {
            self.sink = None;
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_copies_all_bytes_into_sink() {
        let source = std::io::Cursor::new(b"hello world".to_vec());
        let mut sink = Vec::new();
        let mut tee = TeeReader::new(source, &mut sink);

        let mut out = String::new();
        tee.read_to_string(&mut out).unwrap();

        assert_eq!(out, "hello world");
        drop(tee);
        assert_eq!(sink, b"hello world");
    }

    #[test]
    fn read_continues_when_sink_write_fails() {
        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("boom"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let source = std::io::Cursor::new(b"data".to_vec());
        let mut tee = TeeReader::new(source, FailingWriter);

        let mut out = String::new();
        let result = tee.read_to_string(&mut out);

        assert!(result.is_ok());
        assert_eq!(out, "data");
    }
}
