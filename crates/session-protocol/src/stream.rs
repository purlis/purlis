//! Which kind of stream the host opened: the first byte says, and the client reads it before
//! anything else of the stream.
//!
//! A view ([`crate::view`]) and a timeline ([`crate::timeline`]) ride the same link, each on a
//! stream of its own, and the host opens them in any order. The client takes each stream the
//! link accepts, reads its kind here, and hands it on to the module that reads the rest.

use tokio::io::AsyncReadExt;

use crate::link::{LinkError, Stream};
use crate::timeline::TIMELINE;
use crate::view::VIEW;

/// What a stream carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A terminal's bytes ([`crate::view`]).
    View,
    /// A chat's story ([`crate::timeline`]).
    Timeline,
}

/// A stream whose kind has been read, and nothing else.
#[derive(Debug)]
pub struct Opened {
    kind: Kind,
    stream: Stream,
}

impl Opened {
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The stream, with its first byte gone: for the module its kind names.
    pub(crate) fn into_stream(self) -> Stream {
        self.stream
    }
}

/// Read a stream's kind. A first byte this version does not know is refused with its value,
/// and the stream is dropped: it is from a later version, and nothing here can read it.
pub async fn take(mut stream: Stream) -> Result<Opened, LinkError> {
    let kind = match stream.read_u8().await? {
        VIEW => Kind::View,
        TIMELINE => Kind::Timeline,
        other => {
            return Err(LinkError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("a stream that opens with {other:#04x} is neither a view nor a timeline"),
            )));
        }
    };
    Ok(Opened { kind, stream })
}
