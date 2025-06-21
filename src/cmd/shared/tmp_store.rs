use std::cmp::Ordering;
use std::fs::{remove_file, File};
use std::hash::{Hash, Hasher};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::marker::PhantomData;
use std::mem::size_of_val;
use std::path::{Path, PathBuf};

use byteorder::{ReadBytesExt, LE};
use deepsize::DeepSizeOf;
use rkyv::{
    ser::{
        serializers::{AlignedSerializer, AllocScratch, CompositeSerializer},
        Serializer,
    },
    AlignedVec, Archive, Deserialize, Infallible, Serialize,
};
use tempfile::TempDir;

use crate::error::{CliError, CliResult};
use crate::helpers::value::SimpleValue;

pub use super::key::Key;

/// Warning limit for number of temporary files
const TEMP_FILE_WARN_LIMIT: usize = 50;

pub trait Archivable<'a>:
    Archive + Serialize<CompositeSerializer<AlignedSerializer<&'a mut AlignedVec>, AllocScratch>>
{
}

impl Archivable<'_> for Vec<u8> {}
impl Archivable<'_> for Box<[u8]> {}
impl Archivable<'_> for SimpleValue {}

/// Item used in sort and unique commands:
/// holds a key and a formatted record,
/// but only the key is used for comparisons.
#[derive(Archive, Deserialize, Serialize, DeepSizeOf, Debug, Clone)]
#[archive(compare(PartialEq), check_bytes)]
pub struct Item<R: for<'a> Archivable<'a> + DeepSizeOf> {
    pub key: Key,
    pub record: R,
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> Item<R> {
    pub fn new(key: Key, record: R) -> Self {
        Self { key, record }
    }
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> PartialOrd for Item<R> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> PartialEq for Item<R> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> Eq for Item<R> {}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> Ord for Item<R> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> Hash for Item<R> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl<R: for<'a> Archivable<'a> + DeepSizeOf> Archivable<'_> for Item<R> {}

#[derive(Debug)]
pub struct TmpStore {
    tmp_dir: TempDir,
    num: usize,
    file_limit: usize,
}

impl TmpStore {
    pub fn new(tmp_dir: PathBuf, prefix: &str, file_limit: usize) -> io::Result<Self> {
        Ok(Self {
            tmp_dir: TempDir::with_prefix_in(prefix, tmp_dir)?,
            num: 0,
            file_limit,
        })
    }

    pub fn writer<T>(&mut self, quiet: bool) -> CliResult<TmpWriter<T>> {
        if self.num == TEMP_FILE_WARN_LIMIT && !quiet {
            eprintln!(
                "Warning: sequence sorting resulted in many temporary files ({}). \
                Consider increasing the memory limit (-M/--max-mem). \
                Supply -q/--quiet to silence this warning.",
                TEMP_FILE_WARN_LIMIT
            )
        }
        if self.num == self.file_limit {
            return fail!(
                "Too many temporary files ({}) created by sort command. \
                Try a higher memory limit (-M/--max-mem)",
                self.file_limit
            );
        }
        let new_path = self.tmp_dir.path().join(format!("tmp_{}", self.num));
        self.num += 1;
        TmpWriter::new(&new_path).map_err(CliError::from)
    }
}

#[derive(Debug)]
pub struct TmpWriter<T> {
    path: PathBuf,
    inner: BufWriter<File>,
    buf: AlignedVec,
    scratch: Option<AllocScratch>,
    n_written: usize,
    _t: PhantomData<T>,
}

impl<T> TmpWriter<T> {
    pub fn new(path: &Path) -> io::Result<Self> {
        Ok(Self {
            path: path.to_owned(),
            inner: BufWriter::new(File::create(path)?),
            buf: AlignedVec::new(),
            scratch: Some(AllocScratch::default()),
            n_written: 0,
            _t: PhantomData,
        })
    }

    pub fn write(&mut self, item: &T) -> io::Result<()>
    where
        T: for<'a> Archivable<'a>,
    {
        self.buf.clear();
        let mut serializer = CompositeSerializer::new(
            AlignedSerializer::new(&mut self.buf),
            self.scratch.take().unwrap(),
            Infallible,
        );
        serializer.serialize_value(item).unwrap();
        let (serializer, scratch, _) = serializer.into_components();
        self.scratch = Some(scratch);
        let buf = serializer.into_inner();
        self.n_written += size_of_val(&buf.len()) + buf.len();
        self.inner.write_all(&buf.len().to_le_bytes())?;
        self.inner.write_all(buf)
    }

    pub fn done(mut self) -> io::Result<TmpHandle<T>> {
        self.inner.get_mut().sync_all()?;
        Ok(TmpHandle {
            path: self.path.clone(),
            exp_len: self.n_written,
            _t: PhantomData,
        })
    }
}

#[derive(Debug)]
pub struct TmpHandle<T> {
    path: PathBuf,
    exp_len: usize,
    _t: PhantomData<T>,
}

pub struct TmpReader<T> {
    path: PathBuf,
    inner: BufReader<File>,
    buf: Vec<u8>,
    _t: PhantomData<T>,
}

impl<T> TmpHandle<T> {
    pub fn reader(&self) -> io::Result<TmpReader<T>> {
        TmpReader::new(&self.path, self.exp_len)
    }
}

impl<T> TmpReader<T> {
    pub fn new(path: &Path, exp_len: usize) -> io::Result<Self> {
        let f = File::open(path)?;
        assert_eq!(
            f.metadata().unwrap().len(),
            exp_len as u64,
            "Temporary file length mismatch, data corruption possible."
        );
        Ok(Self {
            path: path.to_owned(),
            inner: BufReader::new(f),
            buf: Vec::new(),
            _t: PhantomData,
        })
    }
}

impl<T> Iterator for TmpReader<T>
where
    T: Archive,
    // for<'a> T::Archived: CheckBytes<DefaultValidator<'a>> + Deserialize<T, Infallible>,
    for<'a> T::Archived: Deserialize<T, Infallible>,
{
    type Item = io::Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let len: u64 = match self.inner.read_u64::<LE>() {
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return None,
            res => try_opt!(res),
        };
        self.buf.clear();
        self.buf.resize(len as usize, 0);
        try_opt!(self.inner.read_exact(&mut self.buf));
        // unsafe appears to save ~ 25% of time
        // we do a basic file size check (in constructor above) to have at least some validation
        let archived = unsafe { rkyv::archived_root::<T>(&self.buf[..]) };
        // let archived = rkyv::check_archived_root::<T>(&self.buf[..]).unwrap();
        let item: T = archived.deserialize(&mut Infallible).unwrap();
        Some(Ok(item))
    }
}

impl<T> TmpReader<T> {
    pub fn done(self) -> io::Result<()> {
        drop(self.inner);
        remove_file(&self.path)
    }
}

// let mut compr_writer = lz4::EncoderBuilder::new()
//     .build(bufwriter)?;
// self.n_written += self.mem_sorter.serialize_sorted(&mut compr_writer)?;
// let (mut bufwriter, res) = compr_writer.finish();
// res?;
// bufwriter.get_mut().sync_all()?;

// let wtr = File::create(&new_path)?;
// let wtr = lz4::EncoderBuilder::new().build(wtr)?;
// let (mut writer, res) = thread_io::write::writer_finish(
//     1 << 22,
//     4,
//     wtr,
//     |w| self.mem_sorter.serialize_sorted(w),
//     |w| w.finish(),
// )?.1;
// writer.sync_all()?;
