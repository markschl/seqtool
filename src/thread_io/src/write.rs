//! Wrap a writer in a background thread.

use std::mem::replace;
use std::io::{self, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use super::crossbeam;


#[derive(Debug)]
enum Message {
    Buffer(io::Cursor<Box<[u8]>>),
    Flush,
    Done
}

#[derive(Debug)]
pub struct Writer {
    empty_recv: Receiver<io::Result<Box<[u8]>>>,
    full_send: Sender<Message>,
    buffer: io::Cursor<Box<[u8]>>,
}

impl Writer {
    fn send_to_thread(&mut self) -> io::Result<()> {
        if let Ok(empty) = self.empty_recv.recv() {
            let full = replace(&mut self.buffer, io::Cursor::new(empty?));
            if self.full_send.send(Message::Buffer(full)).is_err() {
                self.get_errors()?;
            }
        } else {
            self.get_errors()?;
        }
        Ok(())
    }

    fn done(&mut self) -> io::Result<()> {
        // send last buffer
        self.send_to_thread()?;
        self.full_send.send(Message::Done).ok();
        Ok(())
    }

    // return errors that may still be in the queue
    fn get_errors(&self) -> io::Result<()> {
        for res in &self.empty_recv {
            res?;
        }
        Ok(())
    }
}

impl Write for Writer {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut written = 0;
        while written < buffer.len() {
            let n = self.buffer.write(&buffer[written..])?;
            written += n;
            if n == 0 {
                self.send_to_thread()?;
            }
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.full_send.send(Message::Flush).ok();
        Ok(())
    }
}


/// Sends `writer` to a new thread while it provides a writer within a closure
/// in the main thread that doesn't block.
/// **Note**: Errors will not be returned immediately, but after `queuelen`
/// writes, or after writing is finished and the closure ends.
/// Also note that the last `write()` might be done **after** the closure
/// has ended, so calling `flush` within the closure is too early.
/// In that case, flushing (or any method for finalizing after the last
/// write) can be called in the `finish` closure supplied to `writer_with_finish()`.
pub fn writer<W, F, O, E>(bufsize: usize, queuelen: usize, writer: W, func: F) -> Result<O, E>
where
    F: FnOnce(&mut Writer) -> Result<O, E>,
    W: Write + Send,
    E: Send + From<io::Error>
{
    writer_init(bufsize, queuelen, || Ok(writer), func)
}

/// Like `writer()` with the difference that the writer is initialized within
/// the thread using a closure (`init_writer`). This allows using writers
/// that don't implement `Send`
pub fn writer_init<W, I, F, O, E>(
    bufsize: usize,
    queuelen: usize,
    init_writer: I,
    func: F,
) -> Result<O, E>
where
    I: Send + FnOnce() -> Result<W, E>,
    F: FnOnce(&mut Writer) -> Result<O, E>,
    W: Write,
    E: Send + From<io::Error>
{
    writer_init_finish(bufsize, queuelen, init_writer, func, |_| ()).map(|(o, _)| o)
}

/// Like `writer_with()`, but takes another closure that takes the writer by value
/// before it goes out of scope (and there is no error). Useful e.g. with encoders
/// for compressed data that require calling a `finish` function.
pub fn writer_init_finish<W, I, F, O, F2, O2, E>(
    bufsize: usize,
    queuelen: usize,
    init_writer: I,
    func: F,
    finish: F2
) -> Result<(O, O2), E>
where
    I: Send + FnOnce() -> Result<W, E>,
    F: FnOnce(&mut Writer) -> Result<O, E>,
    W: Write,
    F2: Send + FnOnce(W) -> O2,
    O2: Send,
    E: Send + From<io::Error>
{
    assert!(queuelen >= 1);
    assert!(bufsize > 0);

    let (full_send, full_recv): (Sender<Message>, _) =
        channel();
    let (empty_send, empty_recv) = channel();
    for _ in 0..queuelen {
        empty_send
            .send(Ok(vec![0; bufsize].into_boxed_slice()))
            .ok();
    }

    crossbeam::scope(|scope| {
        let handle = scope.spawn::<_, Result<_, E>>(move || {
            let mut writer = init_writer()?;

            while let Ok(msg) = full_recv.recv() {
                match msg {
                    Message::Buffer(buf) => {
                        let pos = buf.position() as usize;
                        let buffer = buf.into_inner();
                        let res = writer.write_all(&buffer[..pos]);
                        let is_err = res.is_err();
                        empty_send.send(res.map(|_| buffer)).ok();
                        if is_err {
                            return Ok(None);
                        }
                    }
                    Message::Flush => {
                        if let Err(e) = writer.flush() {
                            empty_send.send(Err(e)).ok();
                            return Ok(None);
                        }
                    }
                    Message::Done => break
                }
            }
            // writing finished witout error
            Ok(Some(finish(writer)))
        });

        let mut writer = Writer {
            empty_recv: empty_recv,
            full_send: full_send,
            buffer: io::Cursor::new(vec![0; bufsize].into_boxed_slice()),
        };

        let out = func(&mut writer)?;

        writer.done()?;

        let of = handle.join()?;

        writer.get_errors()?;

        Ok((out, of.unwrap()))
    })
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::min;

    #[derive(Clone)]
    struct Writer {
        cache: Vec<u8>,
        data: Vec<u8>,
        write_fails: bool,
        flush_fails: bool,
        bufsize: usize,
    }

    impl Writer {
        fn new(write_fails: bool, flush_fails: bool, bufsize: usize) -> Writer {
            Writer {
                cache: vec![],
                data: vec![],
                write_fails: write_fails,
                flush_fails: flush_fails,
                bufsize: bufsize,
            }
        }
    }

    impl Write for Writer {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if self.write_fails {
                return Err(io::Error::new(io::ErrorKind::Other, "write err"))
            }
            self.cache.write(&buffer[..min(buffer.len(), self.bufsize)])
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.flush_fails {
                Err(io::Error::new(io::ErrorKind::Other, "flush err"))
            } else {
                self.data.extend_from_slice(&self.cache);
                self.cache.clear();
                Ok(())
            }
        }
    }

    #[test]
    fn write_thread() {
        let text = b"The quick brown fox jumps over the lazy dog";
        let len = text.len();

        for channel_bufsize in 1..len {
            for writer_bufsize in 1..len {
                for queuelen in 1..len {
                    // without flushing
                    let mut w = writer_init_finish(channel_bufsize, queuelen,
                        || Ok(Writer::new(false, false, writer_bufsize)),
                        |w| w.write(text),
                        |w| w
                    ).unwrap().1;
                    assert_eq!(&w.data, b"");

                    // with flushing
                    let mut w = writer_init_finish(channel_bufsize, queuelen,
                        || Ok(Writer::new(false, false, writer_bufsize)),
                        |w| w.write(text),
                        |mut w| {
                            w.flush().unwrap();
                            w
                        }).unwrap().1;
                    if w.data.as_slice() != &text[..] {
                        panic!(format!(
                            "write test failed: {:?} != {:?} at channel buffer size {}, writer bufsize {}, queue length {}",
                            String::from_utf8_lossy(&w.data), String::from_utf8_lossy(&text[..]),
                            channel_bufsize, writer_bufsize, queuelen
                        ));
                    }


                    w.flush().unwrap();
                    if w.data.as_slice() != &text[..] {
                        panic!(format!(
                            "write test failed: {:?} != {:?} at channel buffer size {}, writer bufsize {}, queue length {}",
                            String::from_utf8_lossy(&w.data), String::from_utf8_lossy(&text[..]),
                            channel_bufsize, writer_bufsize, queuelen
                        ));
                    }
                }
            }
        }
    }

    #[test]
    fn writer_init_fail() {
        let e = io::Error::new(io::ErrorKind::Other, "init err");
        let res = writer_init(5, 2, || Err::<&mut [u8], _>(e), |_| {Ok(())});
        if let Err(e) = res {
            assert_eq!(&format!("{}", e), "init err");
        } else {
            panic!("init should fail");
        }
    }

    #[test]
    fn write_fail() {
        let text = b"The quick brown fox jumps over the lazy dog";
        let len = text.len();

        for channel_bufsize in 1..len {
            for writer_bufsize in 1..len {
                for queuelen in 1..len {
                    let w = Writer::new(true, false, writer_bufsize);
                    let res = writer(channel_bufsize, queuelen, w, |w| w.write(text));
                    if let Err(e) = res {
                        assert_eq!(&format!("{}", e), "write err");
                    } else {
                        panic!("write should fail");
                    }

                    let w = Writer::new(false, true, writer_bufsize);
                    let res = writer(channel_bufsize, queuelen, w, |w| w.flush());
                    if let Err(e) = res {
                        assert_eq!(&format!("{}", e), "flush err");
                    } else {
                        panic!("flush should fail");
                    }
                }
            }
        }
    }
}
