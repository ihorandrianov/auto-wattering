//! Watering system with async OLED and BME280

#![no_std]
#![no_main]

mod channels;
mod config;
mod tasks;
mod types;

use cyw43::JoinOptions;
use cyw43_pio::PioSpi;
use fixed::FixedU32;
use fixed::types::extra::U8;

// Custom clock divider for Pico 2 W - 0x0300
const PICO2W_CLOCK_DIVIDER: FixedU32<U8> = FixedU32::from_bits(0x0300);

use embassy_executor::Spawner;
use embassy_net::{Config, StackResources};
use embassy_rp::adc::{Adc, Channel, InterruptHandler as AdcInterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_rp::block::ImageDef;
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Input, Level, Output};
use embassy_rp::i2c::{self, Config as I2cConfig, InterruptHandler as I2cInterruptHandler};
use embassy_rp::peripherals::{I2C1, PIO0, USB};
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

#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: ImageDef = ImageDef::secure_exe();

bind_interrupts!(struct Irqs {
    I2C1_IRQ => I2cInterruptHandler<I2C1>;
    USBCTRL_IRQ => UsbInterruptHandler<USB>;
    PIO0_IRQ_0 => PioInterruptHandler<PIO0>;
    ADC_IRQ_FIFO => AdcInterruptHandler;
});

#[unsafe(link_section = ".bi_entries")]
#[used]
pub static PICOTOOL_ENTRIES: [embassy_rp::binary_info::EntryAddr; 4] = [
    embassy_rp::binary_info::rp_program_name!(c"Watering System"),
    embassy_rp::binary_info::rp_program_description!(c"Auto watering with BME280 and OLED"),
    embassy_rp::binary_info::rp_cargo_version!(),
    embassy_rp::binary_info::rp_program_build_attribute!(),
];

pub type I2cBus = Mutex<CriticalSectionRawMutex, i2c::I2c<'static, I2C1, i2c::Async>>;
static I2C_BUS: StaticCell<I2cBus> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut rng = RoscRng;

    // === USB Logger ===
    let driver = Driver::new(p.USB, Irqs);
    spawner.spawn(logger::logger_task(driver)).unwrap();
    Timer::after_millis(500).await;

    Timer::after_millis(100).await;

    info!("Loading CYW43 firmware");
    Timer::after_millis(100).await;
    let fw = include_bytes!("../firmware/43439A0.bin");
    let clm = include_bytes!("../firmware/43439A0_clm.bin");

    info!("Setting up PIO SPI");
    Timer::after_millis(100).await;
    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        PICO2W_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        p.DMA_CH0,
    );

    info!("Initializing ADC");
    let adc = Adc::new(p.ADC, Irqs, embassy_rp::adc::Config::default());
    let soil_pin = Channel::new_pin(p.PIN_28, embassy_rp::gpio::Pull::None);

    info!("Initializing sonar");
    let sonar_trigger = Output::new(p.PIN_16, Level::Low);
    let sonar_echo = Input::new(p.PIN_17, embassy_rp::gpio::Pull::None);

    info!("Initializing CYW43");
    static CYW43_STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = CYW43_STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    info!("CYW43 initialized!");

    spawner.spawn(network::cyw43_task(runner)).unwrap();

    info!("Loading CLM");
    control.init(clm).await;
    info!("CLM loaded!");

    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let config = Config::dhcpv4(Default::default());
    let seed = rng.next_u64();

    static RESOURCES: StaticCell<StackResources<5>> = StaticCell::new();
    let (stack, net_runner) = embassy_net::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );

    spawner.spawn(network::net_task(net_runner)).unwrap();

    let sda = p.PIN_26;
    let scl = p.PIN_27;
    let i2c = i2c::I2c::new_async(p.I2C1, scl, sda, Irqs, I2cConfig::default());
    let i2c_bus = I2C_BUS.init(Mutex::new(i2c));

    info!("I2C bus initialized on GP26/GP27");

    spawner.spawn(display::display_task(i2c_bus)).unwrap();
    spawner
        .spawn(sensor::sensor_task(
            i2c_bus,
            adc,
            soil_pin,
            sonar_trigger,
            sonar_echo,
        ))
        .unwrap();

    info!("Connecting to WiFi...");
    while let Err(err) = control
        .join(WIFI_NETWORK, JoinOptions::new(WIFI_PASSWORD.as_bytes()))
        .await
    {
        info!("WiFi join failed: {:?}", err);
        Timer::after_secs(1).await;
    }
    info!("WiFi connected!");

    spawner.spawn(network::http_task(stack, seed)).unwrap();
    spawner.spawn(network::poll_task()).unwrap();

    info!("All tasks spawned");
}
