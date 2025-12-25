use core::fmt::Write;

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_time::Timer;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use heapless::String;
use log::info;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306Async};

use crate::channels::SENSOR_CHANNEL;
use crate::I2cBus;

#[embassy_executor::task]
pub async fn display_task(i2c_bus: &'static I2cBus) {
    Timer::after_millis(50).await;

    let i2c_dev = I2cDevice::new(i2c_bus);
    let interface = I2CDisplayInterface::new(i2c_dev);
    let mut display = Ssd1306Async::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    if display.init().await.is_err() {
        info!("Failed to init OLED!");
        loop {
            Timer::after_secs(10).await;
        }
    }

    info!("OLED initialized!");

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    display.clear_buffer();
    Text::with_baseline("Waiting for", Point::new(10, 10), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline("sensor data...", Point::new(10, 25), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().await.ok();

    loop {
        let data = SENSOR_CHANNEL.receive().await;

        display.clear_buffer();

        let mut s: String<32> = String::new();
        write!(s, "Temp: {}C", data.temperature as i32).unwrap();
        Text::with_baseline(&s, Point::new(5, 5), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        s.clear();
        write!(s, "Humidity: {}%", data.humidity as i32).unwrap();
        Text::with_baseline(&s, Point::new(5, 20), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        s.clear();
        write!(s, "Pressure: {}hPa", data.pressure as i32).unwrap();
        Text::with_baseline(&s, Point::new(5, 35), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        Text::with_baseline("System OK", Point::new(5, 52), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();

        if display.flush().await.is_err() {
            info!("Display flush error");
        }
    }
}
