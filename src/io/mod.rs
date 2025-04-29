pub use self::format::*;
pub use self::qual_format::*;
pub use self::record::*;

mod format;
pub mod input;
pub mod output;
mod qual_format;
mod record;

pub const DEFAULT_FORMAT: FormatVariant = FormatVariant::Fasta;

pub const DEFAULT_IO_READER_BUFSIZE: usize = 1 << 22;
pub const DEFAULT_IO_WRITER_BUFSIZE: usize = 1 << 22;
