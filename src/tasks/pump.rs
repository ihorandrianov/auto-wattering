use embassy_rp::gpio::Output;
use embassy_time::Timer;
use log::info;

use crate::channels::PUMP_CHANNEL;
use crate::config::PUMP_MAX_DURATION_SECS;

#[embassy_executor::task]
pub async fn pump_task(mut pump_pin: Output<'static>) {
    info!("Pump task started");

    loop {
        let cmd = PUMP_CHANNEL.receive().await;

        let duration = cmd.duration_secs.min(PUMP_MAX_DURATION_SECS);
        info!("Pump ON for {} secs", duration);

        pump_pin.set_high();
        Timer::after_secs(duration as u64).await;
        pump_pin.set_low();

        info!("Pump OFF");
    }
}
