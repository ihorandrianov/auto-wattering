use bme280_rs::{AsyncBme280, Configuration, Oversampling, SensorMode};
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_time::Timer;
use log::info;

use crate::channels::SENSOR_CHANNEL;
use crate::config::SENSOR_INTERVAL_MS;
use crate::types::SensorData;
use crate::I2cBus;

#[embassy_executor::task]
pub async fn sensor_task(i2c_bus: &'static I2cBus) {
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

    loop {
        Timer::after_millis(SENSOR_INTERVAL_MS).await;

        let temp = bme280.read_temperature().await;
        let hum = bme280.read_humidity().await;
        let press = bme280.read_pressure().await;

        match (temp, hum, press) {
            (Ok(Some(t)), Ok(Some(h)), Ok(Some(p))) => {
                let data = SensorData {
                    temperature: t,
                    humidity: h,
                    pressure: p / 100.0,
                };

                info!(
                    "T: {}C, H: {}%, P: {}hPa",
                    data.temperature as i32,
                    data.humidity as i32,
                    data.pressure as i32
                );

                SENSOR_CHANNEL.try_send(data).ok();
            }
            _ => {
                info!("BME280 read error");
            }
        }
    }
}
