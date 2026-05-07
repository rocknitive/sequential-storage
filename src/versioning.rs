use core::ops::Range;

use embedded_storage_async::nor_flash::NorFlash;

use crate::{
    AlignedBuf, Error, GenericStorage, MARKER, MARKER_SET_BITS, MAX_WORD_SIZE, NorFlashExt,
};
use crate::{cache::CacheImpl, calculate_page_address};

const PAGE_START_HEADER_MARKER_INDEX: usize = 0;
const PAGE_START_HEADER_INTERNAL_VERSION_INDEX: usize = 1;
const PAGE_START_HEADER_USER_VERSION_RANGE: Range<usize> = 2..4;
const PAGE_START_HEADER_SIZE: usize = 4;

const FLASH_FORMAT_VERSION: u8 = {
    const BASE_VERSION: u8 = 1;
    #[cfg(feature = "tombstone")]
    {
        (BASE_VERSION << 1) | 1
    }
    #[cfg(not(feature = "tombstone"))]
    {
        BASE_VERSION << 1
    }
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// The kind of version mismatch that was detected in flash.
pub enum VersionMismatchKind {
    /// The crate's internal flash format version differs.
    Internal {
        /// The internal flash format version expected by the running firmware.
        expected: u8,
        /// The internal flash format version decoded from flash.
        actual: u8,
    },
    /// The user supplied storage version differs.
    User {
        /// The user supplied storage version expected by the running firmware.
        expected: u16,
        /// The user supplied storage version decoded from flash.
        actual: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// How `verify()` should handle mismatches.
pub enum VersionPolicy {
    /// Return an error if a mismatch is found.
    ErrorOnMismatch,
    /// Erase the full flash range if a mismatch is found.
    EraseOnMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StorageVersionInfo {
    internal: u8,
    user: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageStartStatus {
    Open,
    Written(StorageVersionInfo),
    Corrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct State {
    user_version: u16,
}

impl State {
    pub(crate) const fn new() -> Self {
        Self { user_version: 0 }
    }

    pub(crate) fn set_user_version(&mut self, user_version: u16) {
        self.user_version = user_version;
    }

    pub(crate) fn page_start_buffer(&self) -> AlignedBuf<MAX_WORD_SIZE> {
        let mut buffer = AlignedBuf([0xFF; MAX_WORD_SIZE]);
        buffer[PAGE_START_HEADER_MARKER_INDEX] = MARKER;
        buffer[PAGE_START_HEADER_INTERNAL_VERSION_INDEX] = FLASH_FORMAT_VERSION;
        buffer[PAGE_START_HEADER_USER_VERSION_RANGE]
            .copy_from_slice(&self.user_version.to_le_bytes());
        buffer
    }
}

pub(crate) const fn page_start_size<S: NorFlash>() -> usize {
    if S::WORD_SIZE < PAGE_START_HEADER_SIZE {
        PAGE_START_HEADER_SIZE
    } else {
        S::WORD_SIZE
    }
}

fn marker_byte_is_set(value: u8) -> bool {
    value.count_zeros() >= MARKER_SET_BITS
}

fn decode_page_start_header(buffer: &[u8]) -> StorageVersionInfo {
    StorageVersionInfo {
        internal: buffer[PAGE_START_HEADER_INTERNAL_VERSION_INDEX],
        user: u16::from_le_bytes(
            buffer[PAGE_START_HEADER_USER_VERSION_RANGE]
                .try_into()
                .unwrap(),
        ),
    }
}

async fn get_page_start_status<S: NorFlash>(
    flash: &mut S,
    offset: u32,
) -> Result<PageStartStatus, Error<S::Error>> {
    let mut buffer = [0xFF; MAX_WORD_SIZE];
    flash
        .read(offset, &mut buffer[..page_start_size::<S>()])
        .await
        .map_err(|e| Error::Storage {
            value: e,
            #[cfg(feature = "_test")]
            backtrace: std::backtrace::Backtrace::capture(),
        })?;

    let written = &buffer[..page_start_size::<S>()];
    if written.iter().all(|byte| *byte == u8::MAX) {
        return Ok(PageStartStatus::Open);
    }

    let marker_written = marker_byte_is_set(written[PAGE_START_HEADER_MARKER_INDEX]);
    let version_bytes_erased = written
        [PAGE_START_HEADER_INTERNAL_VERSION_INDEX..PAGE_START_HEADER_SIZE]
        .iter()
        .all(|byte| *byte == u8::MAX);
    let padding_erased = written[PAGE_START_HEADER_SIZE..]
        .iter()
        .all(|byte| *byte == u8::MAX);

    if !marker_written {
        return Ok(if version_bytes_erased && padding_erased {
            PageStartStatus::Open
        } else {
            PageStartStatus::Corrupted
        });
    }

    if !padding_erased {
        return Ok(PageStartStatus::Corrupted);
    }

    Ok(PageStartStatus::Written(decode_page_start_header(
        &written[..PAGE_START_HEADER_SIZE],
    )))
}

pub(crate) async fn page_start_is_marked<S: NorFlash>(
    flash: &mut S,
    offset: u32,
) -> Result<bool, Error<S::Error>> {
    match get_page_start_status(flash, offset).await? {
        PageStartStatus::Open => Ok(false),
        PageStartStatus::Written(_) => Ok(true),
        PageStartStatus::Corrupted => Err(Error::Corrupted {
            #[cfg(feature = "_test")]
            backtrace: std::backtrace::Backtrace::capture(),
        }),
    }
}

pub(crate) async fn verify_storage<S: NorFlash, C: CacheImpl>(
    storage: &mut GenericStorage<S, C>,
    user_version: u16,
    policy: VersionPolicy,
) -> Result<(), Error<S::Error>> {
    storage.versioning.set_user_version(user_version);
    storage.cache.invalidate_cache_state();

    for page_index in storage.get_pages(0) {
        let page_address = calculate_page_address::<S>(storage.flash_range.clone(), page_index);
        let start_status = get_page_start_status(&mut storage.flash, page_address).await?;

        let PageStartStatus::Written(actual_version) = start_status else {
            if start_status == PageStartStatus::Corrupted {
                return Err(Error::Corrupted {
                    #[cfg(feature = "_test")]
                    backtrace: std::backtrace::Backtrace::capture(),
                });
            }

            continue;
        };

        let mismatch = if actual_version.internal != FLASH_FORMAT_VERSION {
            Some(VersionMismatchKind::Internal {
                expected: FLASH_FORMAT_VERSION,
                actual: actual_version.internal,
            })
        } else if actual_version.user != user_version {
            Some(VersionMismatchKind::User {
                expected: user_version,
                actual: actual_version.user,
            })
        } else {
            None
        };

        if let Some(mismatch) = mismatch {
            return match policy {
                VersionPolicy::ErrorOnMismatch => Err(Error::VersionMismatch(mismatch)),
                VersionPolicy::EraseOnMismatch => {
                    storage.erase_all().await?;
                    storage.cache.invalidate_cache_state();
                    Ok(())
                }
            };
        }
    }

    Ok(())
}

#[cfg(test)]
pub(crate) const fn flash_format_version() -> u8 {
    FLASH_FORMAT_VERSION
}
