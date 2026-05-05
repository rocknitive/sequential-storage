#[cfg(test)]
mod queue_tests {
    use crate::{
        AlignedBuf,
        cache::{CacheImpl, NoCache, PagePointerCache, PageStateCache},
        mock_flash::{self, FlashStatsResult, WriteCountCheck},
        queue::{QueueConfig, QueueStorage},
    };

    use futures_test::test;

    const NUM_PAGES: usize = 4;
    const LOOP_COUNT: usize = 2000;

    #[test]
    async fn cache_reduces_reads() {
        let no_cache = run_test(NoCache::new()).await;
        let page_state = run_test(PageStateCache::<NUM_PAGES>::new()).await;
        let page_pointer = run_test(PagePointerCache::<NUM_PAGES>::new()).await;

        assert!(page_state.reads < no_cache.reads);
        assert!(page_state.bytes_read < no_cache.bytes_read);
        assert!(page_pointer.reads < page_state.reads);
        assert!(page_pointer.bytes_read < page_state.bytes_read);
    }

    async fn run_test(cache: impl CacheImpl) -> FlashStatsResult {
        let mut storage = QueueStorage::new(
            mock_flash::MockFlashBase::<NUM_PAGES, 1, 256>::new(WriteCountCheck::Twice, None, true),
            const { QueueConfig::new(0x00..0x400) },
            cache,
        );
        let mut data_buffer = AlignedBuf([0; 1024]);

        let start_snapshot = storage.flash().stats_snapshot();

        for i in 0..LOOP_COUNT {
            println!("{i}");
            let data = vec![i as u8; i % 20 + 1];

            println!("PUSH");
            storage.push(&data, true).await.unwrap();
            assert_eq!(
                storage.peek(&mut data_buffer).await.unwrap().unwrap(),
                &data,
                "At {i}"
            );
            println!("POP");
            assert_eq!(
                storage.pop(&mut data_buffer).await.unwrap().unwrap(),
                &data,
                "At {i}"
            );
            println!("PEEK");
            assert_eq!(
                storage.peek(&mut data_buffer).await.unwrap(),
                None,
                "At {i}"
            );
            println!("DONE");
        }

        start_snapshot.compare_to(storage.flash().stats_snapshot())
    }
}

#[cfg(test)]
mod map_tests {
    use crate::{
        AlignedBuf,
        cache::{KeyCacheImpl, KeyPointerCache, NoCache, PagePointerCache, PageStateCache},
        map::{MapConfig, MapStorage},
        mock_flash::{self, FlashStatsResult, WriteCountCheck},
    };

    use futures_test::test;

    const NUM_PAGES: usize = 4;

    #[test]
    async fn cache_reduces_reads() {
        let no_cache = run_test(NoCache::new()).await;
        let page_state = run_test(PageStateCache::<NUM_PAGES>::new()).await;
        let page_pointer = run_test(PagePointerCache::<NUM_PAGES>::new()).await;
        let key_pointer_half = run_test(KeyPointerCache::<NUM_PAGES, u16, 12>::new()).await;
        let key_pointer_full = run_test(KeyPointerCache::<NUM_PAGES, u16, 24>::new()).await;

        assert!(page_state.reads < no_cache.reads);
        assert!(page_pointer.reads < page_state.reads);
        assert!(key_pointer_half.reads < page_pointer.reads);
        assert!(key_pointer_full.reads < key_pointer_half.reads);

        assert!(page_state.bytes_read < no_cache.bytes_read);
        assert!(page_pointer.bytes_read < page_state.bytes_read);
        assert!(key_pointer_half.bytes_read < page_pointer.bytes_read);
        assert!(key_pointer_full.bytes_read < key_pointer_half.bytes_read);
    }

    async fn run_test(cache: impl KeyCacheImpl<u16>) -> FlashStatsResult {
        let mut storage = MapStorage::new(
            mock_flash::MockFlashBase::<NUM_PAGES, 1, 256>::new(WriteCountCheck::Twice, None, true),
            const { MapConfig::new(0x00..0x400) },
            cache,
        );
        let mut data_buffer = AlignedBuf([0; 128]);

        const LENGHT_PER_KEY: [usize; 24] = [
            11, 13, 6, 13, 13, 10, 2, 3, 5, 36, 1, 65, 4, 6, 1, 15, 10, 7, 3, 15, 9, 3, 4, 5,
        ];

        let start_snapshot = storage.flash().stats_snapshot();

        for _ in 0..100 {
            const WRITE_ORDER: [usize; 24] = [
                15, 0, 4, 22, 18, 11, 19, 8, 14, 23, 5, 1, 16, 10, 6, 12, 20, 17, 3, 9, 7, 13, 21,
                2,
            ];

            for i in WRITE_ORDER {
                storage
                    .store_item(
                        &mut data_buffer,
                        &(i as u16),
                        &vec![i as u8; LENGHT_PER_KEY[i]].as_slice(),
                    )
                    .await
                    .unwrap();
            }

            const READ_ORDER: [usize; 24] = [
                8, 22, 21, 11, 16, 23, 13, 15, 19, 7, 6, 2, 12, 1, 17, 4, 20, 14, 10, 5, 9, 3, 18,
                0,
            ];

            for i in READ_ORDER {
                let item = storage
                    .fetch_item::<&[u8]>(&mut data_buffer, &(i as u16))
                    .await
                    .unwrap()
                    .unwrap();

                // println!("Fetched {item:?}");

                assert_eq!(item, vec![i as u8; LENGHT_PER_KEY[i]]);
            }
        }

        start_snapshot.compare_to(storage.flash().stats_snapshot())
    }
}
