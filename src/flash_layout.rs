use core::fmt::{Display, Formatter};
use core::{marker::PhantomData, num::NonZeroUsize, ops::Range};
use embedded_storage_async::nor_flash::NorFlash;

use crate::{AlignedBuf, Error, MARKER, MAX_WORD_SIZE, NorFlashExt, marker_is_set};

const PAGE_START_HEADER_MARKER_INDEX: usize = 0;
const PAGE_START_HEADER_INTERNAL_VERSION_INDEX: usize = 1;
const PAGE_START_HEADER_USER_VERSION_RANGE: Range<usize> = 2..4;
const PAGE_START_HEADER_SIZE: usize = 4;

pub const FLASH_FORMAT_VERSION: u8 = {
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

pub(crate) struct FlashLayout<S> {
    start: u32,
    end: u32,
    _flash: PhantomData<S>,
}

impl<S> Clone for FlashLayout<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S> Copy for FlashLayout<S> {}

impl<S: NorFlash> FlashLayout<S> {
    pub(crate) const fn new(flash_range: Range<u32>) -> Self {
        Self {
            start: flash_range.start,
            end: flash_range.end,
            _flash: PhantomData,
        }
    }

    pub(crate) fn page_count(self) -> NonZeroUsize {
        let page_count = (self.end - self.start) as usize / S::ERASE_SIZE;
        NonZeroUsize::new(page_count.max(1)).unwrap()
    }

    pub(crate) fn pages_from(
        self,
        start_page_index: usize,
    ) -> impl DoubleEndedIterator<Item = usize> {
        let page_count = self.page_count();
        (0..page_count.get()).map(move |index| (index + start_page_index) % page_count.get())
    }

    pub(crate) const fn page(self, index: usize) -> FlashPage<S> {
        FlashPage {
            layout: self,
            index,
        }
    }

    pub(crate) const fn page_index(self, address: u32) -> usize {
        (address - self.start) as usize / S::ERASE_SIZE
    }

    pub(crate) fn next_page_index(self, page_index: usize) -> usize {
        (page_index + 1) % self.page_count().get()
    }

    pub(crate) fn previous_page_index(self, page_index: usize) -> usize {
        match page_index.checked_sub(1) {
            Some(new_page_index) => new_page_index,
            None => self.page_count().get() - 1,
        }
    }

    pub(crate) const fn page_data_size(self) -> usize {
        S::ERASE_SIZE - FlashPage::<S>::start_marker_size() - S::WORD_SIZE
    }
}

pub(crate) struct FlashPage<S> {
    layout: FlashLayout<S>,
    index: usize,
}

impl<S> Clone for FlashPage<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S> Copy for FlashPage<S> {}

impl<S: NorFlash> FlashPage<S> {
    pub(crate) const fn start_address(&self) -> u32 {
        self.layout.start + (S::ERASE_SIZE * self.index) as u32
    }

    pub(crate) const fn end_address(&self) -> u32 {
        self.layout.start + (S::ERASE_SIZE * (self.index + 1)) as u32
    }

    pub(crate) const fn start_marker_address(&self) -> u32 {
        self.start_address()
    }

    pub(crate) const fn start_marker_size() -> usize {
        if S::WORD_SIZE < PAGE_START_HEADER_SIZE {
            PAGE_START_HEADER_SIZE
        } else {
            S::WORD_SIZE
        }
    }

    pub(crate) const fn end_marker_address(&self) -> u32 {
        self.end_address() - Self::end_marker_size() as u32
    }

    pub(crate) const fn end_marker_size() -> usize {
        S::WORD_SIZE
    }

    pub(crate) const fn data_start_address(&self) -> u32 {
        self.start_address() + Self::start_marker_size() as u32
    }

    pub(crate) const fn data_end_address(&self) -> u32 {
        self.end_address() - Self::end_marker_size() as u32
    }

    pub(crate) async fn start_is_marked(&self, flash: &mut S) -> Result<bool, Error<S::Error>> {
        match self.get_page_start_status(flash).await? {
            None => Ok(false),
            Some(_) => Ok(true),
        }
    }

    pub(crate) async fn end_is_marked(&self, flash: &mut S) -> Result<bool, Error<S::Error>> {
        marker_is_set(flash, self.end_marker_address()).await
    }

    /// Parses version information from the page start marker.
    ///
    /// Returns `Ok(None)` if the marker is still fully erased, `Ok(StorageVersion)` if parsed
    /// successfully, otherwise `Err(S::Error)`.
    pub(crate) async fn get_page_start_status(
        &self,
        flash: &mut S,
    ) -> Result<Option<StorageVersion>, Error<S::Error>> {
        let mut buffer = [0xFF; MAX_WORD_SIZE];
        flash
            .read(
                self.start_marker_address(),
                &mut buffer[..FlashPage::<S>::start_marker_size()],
            )
            .await
            .map_err(|e| Error::Storage {
                value: e,
                #[cfg(feature = "_test")]
                backtrace: std::backtrace::Backtrace::capture(),
            })?;

        let written = &buffer[..FlashPage::<S>::start_marker_size()];
        if written.iter().all(|byte| *byte == u8::MAX) {
            return Ok(None);
        }

        let marker_written = crate::marker_byte_is_set(written[PAGE_START_HEADER_MARKER_INDEX]);
        let version_bytes_erased = written
            [PAGE_START_HEADER_INTERNAL_VERSION_INDEX..PAGE_START_HEADER_SIZE]
            .iter()
            .all(|byte| *byte == u8::MAX);
        let padding_erased = written[PAGE_START_HEADER_SIZE..]
            .iter()
            .all(|byte| *byte == u8::MAX);

        if !marker_written {
            return if version_bytes_erased && padding_erased {
                Ok(None)
            } else {
                Err(Error::Corrupted {
                    #[cfg(feature = "_test")]
                    backtrace: std::backtrace::Backtrace::capture(),
                })
            };
        }

        if !padding_erased {
            return Err(Error::Corrupted {
                #[cfg(feature = "_test")]
                backtrace: std::backtrace::Backtrace::capture(),
            });
        }

        Ok(Some(StorageVersion {
            internal: written[PAGE_START_HEADER_INTERNAL_VERSION_INDEX],
            user: u16::from_le_bytes(
                written[PAGE_START_HEADER_USER_VERSION_RANGE]
                    .try_into()
                    .unwrap(),
            ),
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageVersion {
    internal: u8,
    user: u16,
}

impl Display for StorageVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "StorageVersion {{ internal: {}, user: {} }}",
            self.internal, self.user
        ))
    }
}

impl StorageVersion {
    pub(crate) const fn new(user: u16) -> Self {
        Self {
            internal: FLASH_FORMAT_VERSION,
            user,
        }
    }

    #[cfg(test)]
    pub const fn with_internal_version(internal: u8, user: u16) -> Self {
        Self { internal, user }
    }

    pub(crate) fn page_start_buffer(&self) -> AlignedBuf<MAX_WORD_SIZE> {
        let mut buffer = AlignedBuf([0xFF; MAX_WORD_SIZE]);
        buffer[PAGE_START_HEADER_MARKER_INDEX] = MARKER;
        buffer[PAGE_START_HEADER_INTERNAL_VERSION_INDEX] = self.internal;
        buffer[PAGE_START_HEADER_USER_VERSION_RANGE].copy_from_slice(&self.user.to_le_bytes());
        buffer
    }
}
