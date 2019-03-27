use super::{HandshakeStream, IoStream};
use crate::{ErrorKind, Result};
use failure::Fail;
use mio::{Evented, Poll, PollOpt, Ready, Token};
use native_tls::{HandshakeError, MidHandshakeTlsStream};
use std::io::{self, Read, Write};

pub struct TlsConnector(native_tls::TlsConnector);

impl TlsConnector {
    pub fn connect<S>(&self, domain: &str, stream: S) -> Result<TlsHandshakeStream<S>>
    where
        S: Read + Write,
    {
        let inner = Some(match self.0.connect(domain, stream) {
            Ok(s) => InnerHandshake::Done(s),
            Err(HandshakeError::WouldBlock(s)) => InnerHandshake::MidHandshake(s),
            Err(HandshakeError::Failure(err)) => Err(err.context(ErrorKind::TlsHandshake))?,
        });
        Ok(TlsHandshakeStream { inner })
    }
}

impl From<native_tls::TlsConnector> for TlsConnector {
    fn from(inner: native_tls::TlsConnector) -> TlsConnector {
        TlsConnector(inner)
    }
}

pub struct TlsHandshakeStream<S> {
    inner: Option<InnerHandshake<S>>,
}

enum InnerHandshake<S> {
    MidHandshake(MidHandshakeTlsStream<S>),
    Done(native_tls::TlsStream<S>),
}

impl<S: Read + Write> InnerHandshake<S> {
    fn get_ref(&self) -> &S {
        match self {
            InnerHandshake::MidHandshake(s) => s.get_ref(),
            InnerHandshake::Done(s) => s.get_ref(),
        }
    }
}

impl<S: Evented + Read + Write + Send + 'static> HandshakeStream for TlsHandshakeStream<S> {
    type Stream = TlsStream<S>;

    fn progress_handshake(&mut self) -> Result<Option<Self::Stream>> {
        let mid_hs = match self.inner.take().unwrap() {
            InnerHandshake::MidHandshake(mid_hs) => mid_hs,
            InnerHandshake::Done(s) => return Ok(Some(TlsStream(s))),
        };

        match mid_hs.handshake() {
            Ok(s) => Ok(Some(TlsStream(s))),
            Err(HandshakeError::WouldBlock(s)) => {
                self.inner = Some(InnerHandshake::MidHandshake(s));
                Ok(None)
            }
            Err(HandshakeError::Failure(err)) => Err(err.context(ErrorKind::TlsHandshake))?,
        }
    }
}

impl<S: Evented + Read + Write> Evented for TlsHandshakeStream<S> {
    #[inline]
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.inner
            .as_ref()
            .unwrap()
            .get_ref()
            .register(poll, token, interest, opts)
    }

    #[inline]
    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.inner
            .as_ref()
            .unwrap()
            .get_ref()
            .reregister(poll, token, interest, opts)
    }

    #[inline]
    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.inner.as_ref().unwrap().get_ref().deregister(poll)
    }
}

pub struct TlsStream<S>(native_tls::TlsStream<S>);

impl<S: Evented + Read + Write + Send + 'static> IoStream for TlsStream<S> {}

impl<S: Read + Write> Read for TlsStream<S> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl<S: Read + Write> Write for TlsStream<S> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<S: Evented + Read + Write> Evented for TlsStream<S> {
    #[inline]
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.0.get_ref().register(poll, token, interest, opts)
    }

    #[inline]
    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        self.0.get_ref().reregister(poll, token, interest, opts)
    }

    #[inline]
    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.0.get_ref().deregister(poll)
    }
}