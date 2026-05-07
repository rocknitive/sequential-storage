use core::{marker::PhantomData, num::NonZeroUsize, ops::Range};

use embedded_storage_async::nor_flash::NorFlash;

use crate::{Error, NorFlashExt, marker_is_set, versioning};

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
        S::ERASE_SIZE - versioning::page_start_size::<S>() - S::WORD_SIZE
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

    pub(crate) const fn start_marker_size(&self) -> usize {
        versioning::page_start_size::<S>()
    }

    pub(crate) const fn end_marker_address(&self) -> u32 {
        self.end_address() - S::WORD_SIZE as u32
    }

    pub(crate) const fn data_start_address(&self) -> u32 {
        self.start_address() + self.start_marker_size() as u32
    }

    pub(crate) const fn data_end_address(&self) -> u32 {
        self.end_address() - S::WORD_SIZE as u32
    }

    pub(crate) async fn start_is_marked(&self, flash: &mut S) -> Result<bool, Error<S::Error>> {
        versioning::page_start_is_marked(flash, self).await
    }

    pub(crate) async fn end_is_marked(&self, flash: &mut S) -> Result<bool, Error<S::Error>> {
        marker_is_set(flash, self.end_marker_address()).await
    }
}
