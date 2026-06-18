#![no_std]
#![no_main]

use core::ops::Range;

use cortex_m_rt::entry;
use defmt::unwrap;
use embedded_storage::nor_flash::MultiwriteNorFlash;
use sequential_storage::{
    cache::{CacheImpl, KeyCacheImpl, KeyPointerCache, PagePointerCache},
    map::{MapConfig, MapStorage},
    queue::{QueueConfig, QueueStorage},
};
use {defmt_rtt as _, panic_probe as _};

const QUEUE_FLASH_RANGE: Range<u32> = 0xF8000..0xFC000;
const MAP_FLASH_RANGE: Range<u32> = 0xFC000..0x100000;

#[entry]
fn main() -> ! {
    let p = embassy_nrf::init(Default::default());

    let flash = embassy_nrf::nvmc::Nvmc::new(p.NVMC);

    let mut queue_storage = QueueStorage::new(
        flash,
        const { QueueConfig::new(QUEUE_FLASH_RANGE, 1) },
        PagePointerCache::<4>::new(),
    );

    run_queue(&mut queue_storage);

    let (flash, _) = queue_storage.destroy();

    let mut map_storage = MapStorage::new(
        flash,
        const { MapConfig::new(MAP_FLASH_RANGE, 1) },
        KeyPointerCache::<4, u8, 8>::new(),
    );

    run_map(&mut map_storage);

    defmt::info!("All went ok!");
    loop {
        cortex_m::asm::bkpt();
    }
}

fn run_queue<E: defmt::Format>(
    storage: &mut QueueStorage<impl MultiwriteNorFlash<Error = E>, impl CacheImpl>,
) {
    unwrap!(storage.erase_all());

    let mut data_buffer = [0; 32];

    unwrap!(storage.push(&[0, 0, 0, 0], false));

    let peeked = unwrap!(storage.peek(&mut data_buffer));

    defmt::assert_eq!(peeked, Some(&mut [0u8, 0, 0, 0][..]));

    let popped = unwrap!(storage.pop(&mut data_buffer));

    defmt::assert_eq!(popped, Some(&mut [0u8, 0, 0, 0][..]));

    let peeked = unwrap!(storage.peek(&mut data_buffer));

    defmt::assert!(peeked.is_none());
}

fn run_map<E: defmt::Format>(
    storage: &mut MapStorage<u8, impl MultiwriteNorFlash<Error = E>, impl KeyCacheImpl<u8>>,
) {
    unwrap!(storage.erase_all());

    let mut data_buffer = [0; 32];

    unwrap!(storage.store_item(&mut data_buffer, &0u8, &0u8));

    unwrap!(storage.store_item(&mut data_buffer, &1u8, &123u32));

    unwrap!(storage.store_item(&mut data_buffer, &2u8, &0.123f32));

    let fetched = unwrap!(storage.fetch_item::<u32>(&mut data_buffer, &3));

    defmt::assert!(fetched.is_none());

    let fetched = unwrap!(storage.fetch_item::<u32>(&mut data_buffer, &1));

    defmt::assert_eq!(fetched, Some(123));
}
