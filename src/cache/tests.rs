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
    async fn no_cache() {
        assert_eq!(
            run_test(NoCache::new()).await,
            if cfg!(feature = "tombstone") {
                FlashStatsResult {
                    erases: 162,
                    reads: 216603,
                    writes: 6325,
                    bytes_read: 818249,
                    bytes_written: 39814,
                }
            } else {
                FlashStatsResult {
                    erases: 150,
                    reads: 164438,
                    writes: 6301,
                    bytes_read: 791993,
                    bytes_written: 53754,
                }
            }
        );
    }

    #[test]
    async fn page_state_cache() {
        assert_eq!(
            run_test(PageStateCache::<NUM_PAGES>::new()).await,
            if cfg!(feature = "tombstone") {
                FlashStatsResult {
                    erases: 162,
                    reads: 119489,
                    writes: 6325,
                    bytes_read: 575464,
                    bytes_written: 39814,
                }
            } else {
                FlashStatsResult {
                    erases: 150,
                    reads: 67444,
                    writes: 6301,
                    bytes_read: 549508,
                    bytes_written: 53754,
                }
            }
        );
    }

    #[test]
    async fn page_pointer_cache() {
        assert_eq!(
            run_test(PagePointerCache::<NUM_PAGES>::new()).await,
            if cfg!(feature = "tombstone") {
                FlashStatsResult {
                    erases: 162,
                    reads: 14021,
                    writes: 6325,
                    bytes_read: 94124,
                    bytes_written: 39814,
                }
            } else {
                FlashStatsResult {
                    erases: 150,
                    reads: 9867,
                    writes: 6301,
                    bytes_read: 88892,
                    bytes_written: 53754,
                }
            }
        );
    }

    async fn run_test(cache: impl CacheImpl) -> FlashStatsResult {
        let mut storage = QueueStorage::new(
            mock_flash::MockFlashBase::<NUM_PAGES, 1, 256>::new(WriteCountCheck::Twice, None, true),
            const { QueueConfig::new(0x00..0x400, 7) },
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
    async fn no_cache() {
        assert_eq!(
            run_test(NoCache::new()).await,
            if cfg!(feature = "tombstone") {
                FlashStatsResult {
                    erases: 459,
                    reads: 719637,
                    writes: 10855,
                    bytes_read: 4695800,
                    bytes_written: 105655,
                }
            } else {
                FlashStatsResult {
                    erases: 409,
                    reads: 513597,
                    writes: 10259,
                    bytes_read: 4549673,
                    bytes_written: 100467,
                }
            }
        );
    }

    #[test]
    async fn page_state_cache() {
        assert_eq!(
            run_test(PageStateCache::<NUM_PAGES>::new()).await,
            if cfg!(feature = "tombstone") {
                FlashStatsResult {
                    erases: 459,
                    reads: 630647,
                    writes: 10855,
                    bytes_read: 4473325,
                    bytes_written: 105655,
                }
            } else {
                FlashStatsResult {
                    erases: 409,
                    reads: 428149,
                    writes: 10259,
                    bytes_read: 4336053,
                    bytes_written: 100467,
                }
            }
        );
    }

    #[test]
    async fn page_pointer_cache() {
        assert_eq!(
            run_test(PagePointerCache::<NUM_PAGES>::new()).await,
            if cfg!(feature = "tombstone") {
                FlashStatsResult {
                    erases: 459,
                    reads: 582075,
                    writes: 10855,
                    bytes_read: 4245371,
                    bytes_written: 105655,
                }
            } else {
                FlashStatsResult {
                    erases: 409,
                    reads: 401986,
                    writes: 10259,
                    bytes_read: 4126749,
                    bytes_written: 100467,
                }
            }
        );
    }

    #[test]
    async fn key_pointer_cache_half() {
        assert_eq!(
            run_test(KeyPointerCache::<NUM_PAGES, u16, 12>::new()).await,
            if cfg!(feature = "tombstone") {
                FlashStatsResult {
                    erases: 459,
                    reads: 493406,
                    writes: 10855,
                    bytes_read: 3596360,
                    bytes_written: 105655,
                }
            } else {
                FlashStatsResult {
                    erases: 409,
                    reads: 324200,
                    writes: 10259,
                    bytes_read: 3330861,
                    bytes_written: 100467,
                }
            }
        );
    }

    #[test]
    async fn key_pointer_cache_full() {
        assert_eq!(
            run_test(KeyPointerCache::<NUM_PAGES, u16, 24>::new()).await,
            if cfg!(feature = "tombstone") {
                FlashStatsResult {
                    erases: 459,
                    reads: 37116,
                    writes: 10855,
                    bytes_read: 270066,
                    bytes_written: 105655,
                }
            } else {
                FlashStatsResult {
                    erases: 409,
                    reads: 23776,
                    writes: 10259,
                    bytes_read: 247302,
                    bytes_written: 100467,
                }
            }
        );
    }

    async fn run_test(cache: impl KeyCacheImpl<u16>) -> FlashStatsResult {
        let mut storage = MapStorage::new(
            mock_flash::MockFlashBase::<NUM_PAGES, 1, 256>::new(WriteCountCheck::Twice, None, true),
            const { MapConfig::new(0x00..0x400, 7) },
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

                assert_eq!(item, vec![i as u8; LENGHT_PER_KEY[i]]);
            }
        }

        start_snapshot.compare_to(storage.flash().stats_snapshot())
    }
}
