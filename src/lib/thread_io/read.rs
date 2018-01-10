//! Wrap a reader in a background thread.

use std::mem::replace;
use std::io::{self, Read, Write};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use super::crossbeam;

#[derive(Debug)]
pub struct Reader {
    full_recv: Receiver<io::Result<Buffer>>,
    empty_send: SyncSender<Buffer>,
    buffer: Buffer,
}

#[derive(Debug)]
struct Buffer {
    data: Box<[u8]>,
    pos: usize,
    end: usize,
}

impl Buffer {
    fn new(size: usize) -> Buffer {
        Buffer {
            data: vec![0; size].into_boxed_slice(),
            pos: 0,
            end: 0,
        }
    }

    fn read(&mut self, mut buf: &mut [u8]) -> usize {
        let n = buf.write(&self.data[self.pos..self.end]).unwrap();
        self.pos += n;
        n
    }

    fn refill<R: Read>(&mut self, mut reader: R) -> io::Result<usize> {
        let n = reader.read(&mut self.data)?;
        self.pos = 0;
        self.end = n;
        Ok(n)
    }
}

impl io::Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.buffer.read(buf);
        if n == 0 {
            let mut data = self.full_recv
                .recv()
                .map_err(|e| io::Error::new(io::ErrorKind::BrokenPipe, e))??;
            let new_n = data.read(buf);
            let old = replace(&mut self.buffer, data);
            self.empty_send.send(old).unwrap();
            return Ok(new_n);
        }
        Ok(n)
    }
}

pub fn reader<R, F, O>(bufsize: usize, queuelen: usize, reader: R, func: F) -> io::Result<O>
where
    F: FnOnce(&mut Reader) -> O,
    R: io::Read + Send,
{
    reader_with(bufsize, queuelen, || Ok(reader), func)
}

pub fn reader_with<R, I, F, E, O>(
    bufsize: usize,
    queuelen: usize,
    init_reader: I,
    func: F,
) -> Result<O, E>
where
    I: Send + FnOnce() -> Result<R, E>,
    F: FnOnce(&mut Reader) -> O,
    R: io::Read,
    E: Send + From<io::Error>,
{
    assert!(queuelen >= 2);

    let (full_send, full_recv): (SyncSender<io::Result<Buffer>>, _) = sync_channel(queuelen);
    let (empty_send, empty_recv): (SyncSender<Buffer>, _) = sync_channel(queuelen);
    for _ in 0..queuelen - 1 {
        empty_send.send(Buffer::new(bufsize)).ok();
    }

    crossbeam::scope(|scope| {
        let handle = scope.spawn::<_, Result<(), E>>(move || {
            let mut reader = init_reader()?;
            while let Ok(mut buffer) = empty_recv.recv() {
                let result = buffer.refill(&mut reader);
                if full_send.send(result.map(|_| buffer)).is_err() {
                    break;
                }
            }
            Ok(())
        });

        let mut reader = Reader {
            full_recv: full_recv,
            empty_send: empty_send,
            buffer: Buffer::new(bufsize),
        };

        let out = func(&mut reader);

        // drop reader -> writer thread will terminate as well because full_recv.recv() fails
        ::std::mem::drop(reader);
        handle.join()?;
        Ok(out)
    })
}
