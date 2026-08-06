use std::io::{BufRead, Write};

use aster_runtime::{
    HOST_PROTOCOL_MAX_LINE_BYTES, HostInboundFrame, HostOutboundFrame, HostProtocolError,
    decode_host_reply,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum HostIoError {
    #[error(transparent)]
    Protocol(HostProtocolError),
    #[error("host protocol input could not be read")]
    Read,
    #[error("host protocol output could not be written")]
    Write,
}

impl HostIoError {
    pub(crate) fn protocol_error(&self) -> HostProtocolError {
        match self {
            Self::Protocol(error) => error.clone(),
            Self::Read => HostProtocolError::MalformedFrame,
            Self::Write => HostProtocolError::WriteFailure,
        }
    }
}

pub(crate) struct HostTransport<R, W> {
    reader: R,
    writer: W,
}

impl<R: BufRead, W: Write> HostTransport<R, W> {
    pub(crate) fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }

    pub(crate) fn read_frame(&mut self) -> Result<HostInboundFrame, HostIoError> {
        let mut line = Vec::new();
        loop {
            let available = self.reader.fill_buf().map_err(|_| HostIoError::Read)?;
            if available.is_empty() {
                return if line.is_empty() {
                    Err(HostIoError::Protocol(HostProtocolError::UnexpectedEof))
                } else {
                    Err(HostIoError::Protocol(HostProtocolError::MalformedFrame))
                };
            }
            if let Some(newline) = available.iter().position(|byte| *byte == b'\n') {
                if line.len().saturating_add(newline) > HOST_PROTOCOL_MAX_LINE_BYTES {
                    return Err(HostIoError::Protocol(HostProtocolError::MalformedFrame));
                }
                line.extend_from_slice(&available[..newline]);
                self.reader.consume(newline + 1);
                break;
            }
            if line.len().saturating_add(available.len()) > HOST_PROTOCOL_MAX_LINE_BYTES {
                return Err(HostIoError::Protocol(HostProtocolError::MalformedFrame));
            }
            let consumed = available.len();
            line.extend_from_slice(available);
            self.reader.consume(consumed);
        }
        let text = std::str::from_utf8(&line)
            .map_err(|_| HostIoError::Protocol(HostProtocolError::MalformedFrame))?;
        decode_host_reply(text).map_err(HostIoError::Protocol)
    }

    pub(crate) fn write_frame(&mut self, frame: &HostOutboundFrame) -> Result<(), HostIoError> {
        serde_json::to_writer(&mut self.writer, frame).map_err(|_| HostIoError::Write)?;
        self.writer
            .write_all(b"\n")
            .map_err(|_| HostIoError::Write)?;
        self.writer.flush().map_err(|_| HostIoError::Write)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use aster_runtime::{HOST_PROTOCOL_MAX_LINE_BYTES, HostProtocolError};

    use super::{HostIoError, HostTransport};

    const ACK: &str = r#"{"schema_version":1,"session_id":"run-1","in_reply_to":0,"kind":"hello_ack","payload":{"protocol":"aster-host","protocol_version":1}}"#;

    #[test]
    fn exactly_one_mebibyte_is_accepted_but_one_more_byte_is_rejected() {
        let mut exact = ACK.as_bytes().to_vec();
        exact.resize(HOST_PROTOCOL_MAX_LINE_BYTES, b' ');
        exact.push(b'\n');
        let mut transport = HostTransport::new(Cursor::new(exact), Vec::new());
        assert!(transport.read_frame().is_ok(), "exact bound is accepted");

        let mut oversized = ACK.as_bytes().to_vec();
        oversized.resize(HOST_PROTOCOL_MAX_LINE_BYTES + 1, b' ');
        oversized.push(b'\n');
        let mut transport = HostTransport::new(Cursor::new(oversized), Vec::new());
        assert!(matches!(
            transport.read_frame(),
            Err(HostIoError::Protocol(HostProtocolError::MalformedFrame))
        ));
    }

    #[test]
    fn invalid_utf8_missing_newline_and_eof_are_controlled() {
        let mut invalid_utf8 = ACK.as_bytes().to_vec();
        invalid_utf8.push(0xff);
        invalid_utf8.push(b'\n');
        let mut transport = HostTransport::new(Cursor::new(invalid_utf8), Vec::new());
        assert!(matches!(
            transport.read_frame(),
            Err(HostIoError::Protocol(HostProtocolError::MalformedFrame))
        ));

        let mut transport = HostTransport::new(Cursor::new(ACK.as_bytes()), Vec::new());
        assert!(matches!(
            transport.read_frame(),
            Err(HostIoError::Protocol(HostProtocolError::MalformedFrame))
        ));

        let mut transport = HostTransport::new(Cursor::new(Vec::<u8>::new()), Vec::new());
        assert!(matches!(
            transport.read_frame(),
            Err(HostIoError::Protocol(HostProtocolError::UnexpectedEof))
        ));
    }
}
