//! Wrap a reader in a background thread.

use std::mem::replace;
use std::io::{self, Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};
use super::crossbeam;

#[derive(Debug)]
struct Buffer {
    data: Box<[u8]>,
    pos: usize,
    end: usize,
    // read returned n = 0 -> EOF
    maybe_finished: bool,
}

impl Buffer {
    fn new(size: usize) -> Buffer {
        assert!(size > 0);
        Buffer {
            data: vec![0; size].into_boxed_slice(),
            pos: 0,
            end: 0,
            maybe_finished: false,
        }
    }

    fn read(&mut self, mut buf: &mut [u8]) -> usize {
        let n = buf.write(&self.data[self.pos..self.end]).unwrap();
        self.pos += n;
        n
    }

    fn refill<R: Read>(&mut self, mut reader: R) -> io::Result<usize> {
        let mut n_read = 0;
        let mut buf = &mut *self.data;
        while !buf.is_empty() {
            let n = reader.read(buf)?;
            if n == 0 {
                self.maybe_finished = true;
                break
            }
            let tmp = buf;
            buf = &mut tmp[n..];
            n_read += n;
        }
        self.pos = 0;
        self.end = n_read;
        Ok(n_read)
    }
}


#[derive(Debug)]
pub struct Reader {
    full_recv: Receiver<io::Result<Buffer>>,
    empty_send: Sender<Option<Buffer>>,
    buffer: Buffer,
}

impl Reader {
    fn done(&self) {
        self.empty_send.send(None).ok();
    }

    // return errors that may still be in the queue
    fn get_errors(&self) -> io::Result<()> {
        for res in &self.full_recv {
            res?;
        }
        Ok(())
    }
}

impl io::Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let n = self.buffer.read(buf);
            if n > 0 {
                return Ok(n);
            } else if self.buffer.maybe_finished {
                return Ok(0);
            } else {
                let data = self.full_recv.recv().ok().unwrap()?;
                let old = replace(&mut self.buffer, data);
                self.empty_send.send(Some(old)).ok();
            }
        }
    }
}

pub fn reader<R, F, O, E>(bufsize: usize, queuelen: usize, reader: R, func: F) -> Result<O, E>
where
    F: FnOnce(&mut Reader) -> Result<O, E>,
    R: io::Read + Send,
    E: Send + From<io::Error>
{
    reader_init(bufsize, queuelen, || Ok(reader), func)
}

pub fn reader_init<R, I, F, O, E>(
    bufsize: usize,
    queuelen: usize,
    init_reader: I,
    func: F,
) -> Result<O, E>
where
    I: Send + FnOnce() -> Result<R, E>,
    F: FnOnce(&mut Reader) -> Result<O, E>,
    R: io::Read,
    E: Send + From<io::Error>
{
    assert!(queuelen >= 1);

    let (full_send, full_recv): (Sender<io::Result<Buffer>>, _) = channel();
    let (empty_send, empty_recv): (Sender<Option<Buffer>>, _) = channel();
    for _ in 0..queuelen {
        empty_send.send(Some(Buffer::new(bufsize))).ok();
    }

    crossbeam::scope(|scope| {
        let handle = scope.spawn(move || {
            let mut reader = init_reader()?;
            while let Ok(Some(mut buffer)) = empty_recv.recv() {
                if let Err(e) = buffer.refill(&mut reader) {
                    let do_break = e.kind() != io::ErrorKind::Interrupted;
                    full_send.send(Err(e)).ok();
                    if do_break {
                        break;
                    }
                } else {
                    full_send.send(Ok(buffer)).ok();
                }
            }
            Ok::<_, E>(())
        });

        let mut reader = Reader {
            full_recv: full_recv,
            empty_send: empty_send,
            buffer: Buffer::new(bufsize),
        };

        let out = func(&mut reader)?;

        reader.done();

        handle.join()?;

        reader.get_errors()?;

        Ok(out)
    })
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read,self};
    use std::cmp::min;

    struct Reader<'a> {
        data: &'a [u8],
        block_size: usize,
        // used to test what happens to errors that are
        // stuck in the queue
        fails_after: usize
    }

    impl<'a> Reader<'a> {
        fn new(data: &'a [u8], block_size: usize, fails_after: usize) -> Reader {
            Reader {
                data: data,
                block_size: block_size,
                fails_after: fails_after
            }
        }
    }

    impl<'a> Read for Reader<'a> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.fails_after == 0 {
                return Err(io::Error::new(io::ErrorKind::Other, "read err"));
            }
            self.fails_after -= 1;
            let amt = min(self.data.len(), min(buf.len(), self.block_size));
            let (a, b) = self.data.split_at(amt);
            buf[..amt].copy_from_slice(a);
            self.data = b;
            Ok(amt)
        }
    }

    fn read_chunks<R: io::Read>(mut rdr: R, chunksize: usize) -> io::Result<Vec<u8>> {
        let mut out = vec![];
        let mut buf = vec![0; chunksize];
        loop {
            let n = rdr.read(buf.as_mut_slice())?;
            out.extend_from_slice(&buf[..n]);
            if n == 0 {
                break;
            }
        }
        Ok(out)
    }

    #[test]
    fn read() {
        let text = b"The quick brown fox";
        let len = text.len();

        for channel_bufsize in 1..len {
            for rdr_block_size in 1..len {
                for out_bufsize in 1..len {
                    for queuelen in 1..len {
                        // test the mock reader itself
                        let mut rdr = Reader::new(text, rdr_block_size, ::std::usize::MAX);
                        assert_eq!(read_chunks(rdr, out_bufsize).unwrap().as_slice(), &text[..]);

                        // test threaded reader
                        let mut rdr = Reader::new(text, rdr_block_size, ::std::usize::MAX);
                        let out = reader(channel_bufsize, queuelen, rdr, |r| read_chunks(r, out_bufsize)).unwrap();

                        if out.as_slice() != &text[..] {
                            panic!(format!(
                                "left != right at channel bufsize: {}, reader bufsize: {}, final reader bufsize {}, queue length: {}\nleft:  {:?}\nright: {:?}",
                                channel_bufsize, rdr_block_size, out_bufsize, queuelen, &out, &text[..]
                            ));
                        }
                    }
                }
            }
        }
    }


    #[test]
    fn read_fail() {
        let text = b"The quick brown fox";
        let len = text.len();

        for channel_bufsize in 1..len {
            for queuelen in 1..len {
                let mut out = vec![0];
                let mut rdr = Reader::new(text, channel_bufsize, len / channel_bufsize);
                let res = reader(channel_bufsize, queuelen, rdr, |r| {
                    while r.read(&mut out)? > 0 {
                    }
                    Ok(())
                });

                if let Err(e) = res {
                    assert_eq!(&format!("{}", e), "read err");
                } else {
                    panic!(format!(
                        "read should fail at bufsize: {}, queue length: {}",
                        channel_bufsize, queuelen
                    ));
                }
            }
        }
    }

    #[test]
    fn reader_init_fail() {
        let e = io::Error::new(io::ErrorKind::Other, "init err");
        let res = reader_init(5, 2, || Err::<&[u8], _>(e), |_| {Ok(())});
        if let Err(e) = res {
            assert_eq!(&format!("{}", e), "init err");
        } else {
            panic!("init should fail");
        }
    }
}
