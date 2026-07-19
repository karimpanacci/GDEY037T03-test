#![no_std]
#![no_main]

use defmt::{info};
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::peripherals::{DMA_CH1, SPI1};
use embassy_rp::pio::{InterruptHandler};
use embassy_rp::spi::{Async, Config, Spi};
use embassy_rp::{bind_interrupts, dma};
use embassy_sync::blocking_mutex::raw::{NoopRawMutex};
use embassy_sync::mutex::Mutex;
use embassy_time::{Delay, Timer};
use static_cell::StaticCell;

#[allow(unused_imports)]
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH1>;
});

pub type Spi1Bus = Mutex<NoopRawMutex, Spi<'static, SPI1, Async>>;

use uc8253::{RefreshMode, UC8253};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    info!("START");

    let spi_config = {
        let mut config = Config::default();
        config.frequency = 100_000;
        config
    };
    let spi = Spi::new_txonly(p.SPI1, p.PIN_14, p.PIN_15, p.DMA_CH1, Irqs, spi_config);

    let busy_in = Input::new(p.PIN_13, Pull::Down);
    let dc = Output::new(p.PIN_12, Level::Low);
    let rst = Output::new(p.PIN_11, Level::High);
    let cs = Output::new(p.PIN_10, Level::High);

    static SPI_BUS: StaticCell<Spi1Bus> = StaticCell::new();
    let spi_bus = SPI_BUS.init(Mutex::new(spi));
    let a = get_static_ref(spi_bus);
    let spi_device = SpiDevice::new(a, cs);
    let display = UC8253::new(spi_device, dc, rst, busy_in, Delay);
    let mut display = display.init(RefreshMode::Full).await.unwrap();
    loop {
        display.write_framebuffer().await.unwrap();
        display.update_display().await.unwrap();

        Timer::after_secs(1).await;
    }
}

fn get_static_ref<T>(a: &'static mut T) -> &'static T {
    a
}
