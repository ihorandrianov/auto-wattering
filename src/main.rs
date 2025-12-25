//! Watering system with async OLED and BME280

#![no_std]
#![no_main]

mod channels;
mod config;
mod tasks;
mod types;

use cyw43::JoinOptions;
use cyw43_pio::{PioSpi, RM2_CLOCK_DIVIDER};
use embassy_executor::Spawner;
use embassy_net::{Config, StackResources};
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, Config as I2cConfig, InterruptHandler as I2cInterruptHandler};
use embassy_rp::peripherals::{DMA_CH0, I2C1, PIO0, USB};
use embassy_rp::pio::{InterruptHandler as PioInterruptHandler, Pio};
use embassy_rp::usb::{Driver, InterruptHandler as UsbInterruptHandler};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use log::info;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use config::{WIFI_NETWORK, WIFI_PASSWORD};
use tasks::{display, logger, network, sensor};

bind_interrupts!(struct Irqs {
    I2C1_IRQ => I2cInterruptHandler<I2C1>;
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    PIO0_IRQ_0 => PioInterruptHandler<PIO0>;
});

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Watering System"),
    embassy_rp::binary_info::rp_program_description!(c"Auto watering with BME280 and OLED"),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

// Shared I2C bus type - exported for tasks
pub type I2cBus = Mutex<CriticalSectionRawMutex, i2c::I2c<'static, I2C1, i2c::Async>>;
static I2C_BUS: StaticCell<I2cBus> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut rng = RoscRng;

    // === USB Logger ===
    let driver = Driver::new(p.USB, Irqs);
    spawner.spawn(logger::logger_task(driver).unwrap());
    Timer::after_millis(500).await;

    info!("=== Watering System Starting ===");

    // === CYW43 WiFi Setup ===
    let fw = cyw43_firmware::CYW43_43439A0;
    let clm = cyw43_firmware::CYW43_43439A0_CLM;

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        RM2_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    static CYW43_STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = CYW43_STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;

    spawner.spawn(network::cyw43_task(runner).unwrap());

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    // === Network Stack ===
    let config = Config::dhcpv4(Default::default());
    let seed = rng.next_u64();

    static RESOURCES: StaticCell<StackResources<5>> = StaticCell::new();
    let (stack, net_runner) = embassy_net::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );

    spawner.spawn(network::net_task(net_runner).unwrap());

    // === Connect to WiFi ===
    info!("Connecting to WiFi...");
    while let Err(err) = control
        .join(WIFI_NETWORK, JoinOptions::new(WIFI_PASSWORD.as_bytes()))
        .await
    {
        info!("WiFi join failed: {:?}", err);
        Timer::after_secs(1).await;
    }

    spawner.spawn(network::http_task(stack, seed).unwrap());

    // === I2C Bus Setup ===
    let sda = p.PIN_26;
    let scl = p.PIN_27;
    let i2c = i2c::I2c::new_async(p.I2C1, scl, sda, Irqs, I2cConfig::default());
    let i2c_bus = I2C_BUS.init(Mutex::new(i2c));

    info!("I2C bus initialized on GP26/GP27");

    // === Spawn Tasks ===
    spawner.spawn(display::display_task(i2c_bus).unwrap());
    spawner.spawn(sensor::sensor_task(i2c_bus).unwrap());

    info!("All tasks spawned");
}
