use embedded_storage_async::nor_flash::NorFlash;

use crate::{AlignedBuf, Error, MARKER, MAX_WORD_SIZE, NorFlashExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct State;

impl State {
    pub(crate) const fn new() -> Self {
        Self
    }

    pub(crate) fn page_start_buffer(&self) -> AlignedBuf<MAX_WORD_SIZE> {
        AlignedBuf([MARKER; MAX_WORD_SIZE])
    }
}

pub(crate) const fn page_start_size<S: NorFlash>() -> usize {
    S::WORD_SIZE
}

pub(crate) async fn page_start_is_marked<S: NorFlash>(
    flash: &mut S,
    offset: u32,
) -> Result<bool, Error<S::Error>> {
    crate::marker_is_set(flash, offset).await
}
