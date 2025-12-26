use bme280_rs::{AsyncBme280, Configuration, Oversampling, SensorMode};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_rp::adc::{Adc, Async, Channel};
use embassy_rp::gpio::{Input, Output};
use embassy_time::Timer;
use log::info;

use crate::I2cBus;
use crate::channels::{HTTP_CHANNEL, SENSOR_CHANNEL};
use crate::config::SENSOR_INTERVAL_MS;
use crate::types::{HttpRequest, SensorData};

pub const SOIL_DRY: u16 = 3550; // air = 0% moisture
pub const SOIL_WET: u16 = 150; // water = 100% moisture

#[embassy_executor::task]
pub async fn sensor_task(
    i2c_bus: &'static I2cBus,
    mut adc: Adc<'static, Async>,
    mut soil_pin: Channel<'static>,
    mut trigger: Output<'static>,
    echo: Input<'static>,
) {
    Timer::after_millis(100).await;

    let i2c_dev = I2cDevice::new(i2c_bus);
    let delay = embassy_time::Delay;

    let mut bme280: AsyncBme280<_, _> = AsyncBme280::new(i2c_dev, delay);

    if bme280.init().await.is_err() {
        info!("Failed to init BME280!");
        loop {
            Timer::after_secs(10).await;
        }
    }

    if bme280
        .set_sampling_configuration(
            Configuration::default()
                .with_temperature_oversampling(Oversampling::Oversample1)
                .with_pressure_oversampling(Oversampling::Oversample1)
                .with_humidity_oversampling(Oversampling::Oversample1)
                .with_sensor_mode(SensorMode::Normal),
        )
        .await
        .is_err()
    {
        info!("Failed to configure BME280!");
    }

    info!("BME280 initialized!");

    Timer::after_millis(10000).await;

    loop {
        // meteo
        let temp = bme280.read_temperature().await;
        let hum = bme280.read_humidity().await;
        let press = bme280.read_pressure().await;

        // soil
        let soil_raw: u16 = adc.read(&mut soil_pin).await.unwrap();
        let clamped = soil_raw.clamp(SOIL_WET, SOIL_DRY);
        let soil_moisture = ((SOIL_DRY - clamped) as f32 / (SOIL_DRY - SOIL_WET) as f32) * 100.0;

        // TODO: sonar is buggy, fix later
        let water_level = 0.0;
        let _ = (&mut trigger, &echo);

        match (temp, hum, press) {
            (Ok(Some(t)), Ok(Some(h)), Ok(Some(p))) => {
                let data = SensorData {
                    temperature: t,
                    humidity: h,
                    pressure: p / 100.0,
                    soil_moisture,
                    water_level,
                };

                info!(
                    "T: {}C, H: {}%, P: {}hPa, SM: {:.2}%, WL: {:.2}cm",
                    data.temperature as i32,
                    data.humidity as i32,
                    data.pressure as i32,
                    data.soil_moisture,
                    data.water_level
                );

                SENSOR_CHANNEL.try_send(data).ok();
                HTTP_CHANNEL
                    .try_send(HttpRequest::PostSensorData(data))
                    .ok();
            }
            _ => {
                info!("BME280 read error");
            }
        }

        Timer::after_millis(SENSOR_INTERVAL_MS).await;
    }
}

async fn measure_distance(trigger: &mut Output<'static>, echo: &Input<'static>) -> Option<f32> {
    trigger.set_high();
    Timer::after_micros(10).await;
    trigger.set_low();

    let start_wait = embassy_time::Instant::now();
    while echo.is_low() {
        if start_wait.elapsed().as_millis() > 30 {
            return None;
        }
    }

    let start = embassy_time::Instant::now();
    while echo.is_high() {
        if start.elapsed().as_millis() > 30 {
            return None;
        }
    }

    let duration_us = start.elapsed().as_micros();
    Some(duration_us as f32 / 58.0)
}

async fn measure_distance_avg(
    trigger: &mut Output<'static>,
    echo: &Input<'static>,
    samples: u8,
) -> Option<f32> {
    let mut total = 0.0;
    let mut valid = 0;

    for _ in 0..samples {
        if let Some(dist) = measure_distance(trigger, echo).await {
            if dist > 2.0 && dist < 400.0 {
                // valid range
                total += dist;
                valid += 1;
            }
        }
        Timer::after_millis(60).await; // wait between pings
    }

    if valid > 0 {
        Some(total / valid as f32)
    } else {
        None
    }
}
