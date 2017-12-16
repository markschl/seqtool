//! Wrap a writer in a background thread.

use std::mem::replace;
use std::io::{self, Write};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use super::crossbeam;

#[derive(Debug)]
pub struct Writer {
    empty_recv: Receiver<io::Result<Box<[u8]>>>,
    full_send: SyncSender<Option<(io::Cursor<Box<[u8]>>, bool)>>,
    buffer: io::Cursor<Box<[u8]>>,
}

impl Writer {
    fn send_to_thread(&mut self, do_flush: bool) -> io::Result<()> {
        if let Ok(empty) = self.empty_recv.recv() {
            let full = replace(&mut self.buffer, io::Cursor::new(empty?));
            self.full_send.send(Some((full, do_flush))).ok();
        }
        Ok(())
    }

    fn done(&mut self) -> io::Result<()> {
        // send last buffer
        self.send_to_thread(true)?;
        self.full_send.send(None).ok();
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
                self.send_to_thread(false)?;
            }
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.send_to_thread(true)
    }
}

fn write_to<W: Write>(data: &[u8], mut writer: W) -> io::Result<()> {
    let mut written = 0;
    while written < data.len() {
        let n = writer.write(&data[written..])?;
        written += n;
    }
    Ok(())
}

pub fn writer<W, F, O>(bufsize: usize, queuelen: usize, writer: W, func: F) -> io::Result<O>
where
    F: FnOnce(&mut Writer) -> O,
    W: Write + Send,
{
    writer_with(bufsize, queuelen, || Ok(writer), func)
}

pub fn writer_with<W, I, F, E, O>(
    bufsize: usize,
    queuelen: usize,
    init_writer: I,
    func: F,
) -> Result<O, E>
where
    I: Send + FnOnce() -> Result<W, E>,
    F: FnOnce(&mut Writer) -> O,
    W: Write,
    E: Send + From<io::Error>,
{
    assert!(queuelen >= 1);

    let (full_send, full_recv): (SyncSender<Option<(io::Cursor<Box<[u8]>>, bool)>>, _) =
        sync_channel(queuelen);
    let (empty_send, empty_recv) = sync_channel(queuelen);
    for _ in 0..queuelen - 1 {
        empty_send
            .send(Ok(vec![0u8; bufsize].into_boxed_slice()))
            .ok();
    }

    crossbeam::scope(|scope| {
        let handle = scope.spawn::<_, Result<(), E>>(move || {
            let mut writer = init_writer()?;

            while let Ok(Some((buffer, do_flush))) = full_recv.recv() {
                let pos = buffer.position() as usize;
                let buffer = buffer.into_inner();
                match write_to(&buffer[..pos], &mut writer) {
                    Ok(n) => n,
                    Err(e) => {
                        empty_send.send(Err(e)).ok();
                        break;
                    }
                }

                let result = if do_flush { writer.flush() } else { Ok(()) };

                if empty_send.send(result.map(|_| buffer)).is_err() {
                    break;
                }
            }
            Ok(())
        });

        let mut writer = Writer {
            empty_recv: empty_recv,
            full_send: full_send,
            buffer: io::Cursor::new(vec![0u8; bufsize].into_boxed_slice()),
        };

        let out = func(&mut writer);
        writer.done()?;

        // drop writer -> writer thread will terminate as well because full_recv.recv() fails
        //::std::mem::drop(writer);
        handle.join()?;
        Ok(out)
    })
}
