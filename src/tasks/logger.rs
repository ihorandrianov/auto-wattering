use embassy_rp::peripherals::USB;
use embassy_rp::usb::Driver;
use embassy_usb_logger::ReceiverHandler;

struct Handler;

impl ReceiverHandler for Handler {
    fn new() -> Self {
        Handler
    }

    async fn handle_data(&self, _data: &[u8]) {}
}

#[embassy_executor::task]
pub async fn logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver, Handler);
}
