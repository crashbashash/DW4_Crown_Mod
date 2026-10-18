//! Copies the original ISO to a new file, streams an optional archive extension,
//! applies in-place rewrites, verifies, and atomically renames onto the output.
//!
//! This is the only module in the engine that writes a file the player cares
//! about, so its order of operations is deliberate: the input is always opened
//! read-only and never written, and every check that could refuse the run
//! (same-file, free space, output already present) happens before a single byte
//! is copied or a `.part` file is created.

use crate::error::{Error, Result};
use crate::region::WriteRegion;
use md5::{Digest, Md5};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Copy buffer size: the 1.4 GB image is streamed in these chunks so the
/// patcher's memory use stays bounded.
const CHUNK: usize = 8 * 1024 * 1024;

/// Which stage of the write is under way.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Phase {
    Inspecting,
    Copying,
    Extending,
    Writing,
    Verifying,
    Hashing,
}

/// Low-level writer counters use phase-local bytes or region counts. The patch
/// entry points map these to estimated overall work out of 10,000.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct Progress {
    pub phase: Phase,
    pub done: u64,
    pub total: u64,
}

/// Controls whether an existing output may be replaced.
pub struct WriteOptions {
    pub overwrite: bool,
}

/// The result of a successful write: the MD5 of the final file and its length.
#[derive(Debug)]
pub struct WriteOutcome {
    pub md5: String,
    pub bytes: u64,
}

/// Refuses when the destination directory has less than `needed` free bytes.
///
/// Kept free of filesystem access so the comparison can be unit-tested with
/// plain numbers.
pub fn ensure_space(needed: u64, free: u64) -> Result<()> {
    if free < needed {
        return Err(Error::NoSpace { needed, free });
    }
    Ok(())
}

/// Copies `input` to `output`, applies every [`WriteRegion`] in place, verifies
/// the result and only then renames it onto `output`.
///
/// `progress` receives phase reports as the copy advances and `verify` is
/// called with the path of the finished `.part` file before it is renamed. The
/// input is never opened for writing.
pub fn write_output(
    input: &Path,
    output: &Path,
    regions: &[WriteRegion],
    opts: &WriteOptions,
    progress: &mut dyn FnMut(Progress),
    verify: &dyn Fn(&Path) -> Result<()>,
) -> Result<WriteOutcome> {
    write_output_extended(input, output, regions, opts, progress, verify, None)
}

/// Optional streamed extension, written to the private part file before verification.
pub type Append<'a> = dyn Fn(&mut File, &mut dyn FnMut(Progress)) -> Result<()> + 'a;

pub struct Extension<'a> {
    pub output_size: u64,
    pub append: &'a Append<'a>,
}

pub fn write_output_extended(
    input: &Path,
    output: &Path,
    regions: &[WriteRegion],
    opts: &WriteOptions,
    progress: &mut dyn FnMut(Progress),
    verify: &dyn Fn(&Path) -> Result<()>,
    extension: Option<Extension<'_>>,
) -> Result<WriteOutcome> {
    // Every refusal that would matter to the player runs first, before anything
    // is created or truncated.
    let input_canon = fs::canonicalize(input)?;
    let output_canon = canonical_output(output)?;
    if input_canon == output_canon {
        return Err(Error::OutputIsInput);
    }

    let input_len = fs::metadata(&input_canon)?.len();
    let parent = output_canon.parent().unwrap_or_else(|| Path::new("."));
    let output_len = extension.as_ref().map_or(input_len, |e| e.output_size);
    ensure_space(output_len, available_space(parent)?)?;

    if !opts.overwrite && output.try_exists()? {
        return Err(Error::OutputExists);
    }

    let part_path = part_path(output);
    progress(Progress {
        phase: Phase::Inspecting,
        done: 0,
        total: input_len,
    });

    if let Err(e) = copy_to_part(input, &part_path, input_len, progress) {
        let _ = fs::remove_file(&part_path);
        return Err(e);
    }
    if let Some(extension) = extension {
        let appended = (|| {
            let mut part = OpenOptions::new().append(true).open(&part_path)?;
            (extension.append)(&mut part, &mut |p| {
                progress(Progress {
                    phase: Phase::Extending,
                    ..p
                })
            })?;
            part.sync_all()?;
            if part.metadata()?.len() != output_len {
                return Err(Error::VerificationFailed(
                    "expanded output size mismatch".into(),
                ));
            }
            Ok(())
        })();
        if let Err(e) = appended {
            let _ = fs::remove_file(&part_path);
            return Err(e);
        }
    }

    let written = apply_regions(&part_path, regions, input_len, progress);
    if let Err(e) = written {
        let _ = fs::remove_file(&part_path);
        return Err(e);
    }

    progress(Progress {
        phase: Phase::Verifying,
        done: 0,
        total: 1,
    });
    if let Err(e) = verify(&part_path) {
        let _ = fs::remove_file(&part_path);
        return Err(e);
    }
    progress(Progress {
        phase: Phase::Verifying,
        done: 1,
        total: 1,
    });

    fs::rename(&part_path, output)?;

    Ok(WriteOutcome {
        md5: md5_hex_with_progress(output, &mut |done, total| {
            progress(Progress {
                phase: Phase::Hashing,
                done,
                total,
            })
        })?,
        bytes: output_len,
    })
}

/// Streams `input` into `part_path` in [`CHUNK`]-sized pieces, creating or
/// truncating the part file so a stale one from an interrupted run is replaced,
/// never read.
fn copy_to_part(
    input: &Path,
    part_path: &Path,
    total: u64,
    progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    let mut src = File::open(input)?;
    let mut dst = File::create(part_path)?;

    progress(Progress {
        phase: Phase::Copying,
        done: 0,
        total,
    });
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    loop {
        let n = src.read(&mut buf)?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n])?;
        done += n as u64;
        progress(Progress {
            phase: Phase::Copying,
            done,
            total,
        });
    }
    Ok(())
}

/// Reopens the finished copy and writes each region by seeking to its offset,
/// so the file is never loaded into memory. Any region that would run past the
/// end of the file is a caller defect and is refused before a partial write.
fn apply_regions(
    part_path: &Path,
    regions: &[WriteRegion],
    file_len: u64,
    progress: &mut dyn FnMut(Progress),
) -> Result<()> {
    let mut part = OpenOptions::new().read(true).write(true).open(part_path)?;

    progress(Progress {
        phase: Phase::Writing,
        done: 0,
        total: regions.len() as u64,
    });
    for (index, region) in regions.iter().enumerate() {
        let end = region
            .offset
            .checked_add(region.bytes.len() as u64)
            .ok_or_else(|| {
                Error::VerificationFailed(format!(
                    "region at offset {} overflows the file offset range",
                    region.offset
                ))
            })?;
        if end > file_len {
            return Err(Error::VerificationFailed(format!(
                "region at offset {} ({} bytes) runs past the end of the {}-byte file",
                region.offset,
                region.bytes.len(),
                file_len
            )));
        }
        part.seek(SeekFrom::Start(region.offset))?;
        part.write_all(&region.bytes)?;
        if (index + 1).is_multiple_of(32) || index + 1 == regions.len() {
            progress(Progress {
                phase: Phase::Writing,
                done: index as u64 + 1,
                total: regions.len() as u64,
            });
        }
    }
    part.sync_all()?;
    Ok(())
}

/// The `.part` file the copy is built in: `<output>.part`.
pub(crate) fn part_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

/// Canonical form of the output path for the same-file check. The output may
/// not exist yet, so its parent directory is canonicalised and the file name is
/// joined back on rather than canonicalising the file itself.
pub(crate) fn canonical_output(output: &Path) -> Result<PathBuf> {
    let file_name = output.file_name().ok_or_else(|| {
        Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the output path has no file name",
        ))
    })?;
    let parent = match output.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    Ok(fs::canonicalize(parent)?.join(file_name))
}

/// Free space in a directory, in bytes, so the space check stays a single
/// query that is easy to swap or mock.
pub(crate) fn available_space(dir: &Path) -> Result<u64> {
    Ok(fs4::available_space(dir)?)
}

/// Hex MD5 of a file, streamed so a 1.4 GB image is not loaded into memory.
pub(crate) fn md5_hex_with_progress(
    path: &Path,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<String> {
    let mut file = File::open(path)?;
    let total = file.metadata()?.len();
    let mut done = 0;
    progress(done, total);
    let mut hasher = Md5::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
        progress(done, total);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
