use cyw43::SpiBus;
use cyw43_pio::PioSpi;
use embassy_net::Runner;
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embassy_rp::gpio::Output;
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use reqwless::client::{HttpClient, TlsConfig};
use static_cell::StaticCell;

use crate::channels::HTTP_CHANNEL;
use crate::types::HttpRequest;

pub type Cyw43Runner =
    cyw43::Runner<'static, SpiBus<Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>>;

#[embassy_executor::task]
pub async fn cyw43_task(runner: Cyw43Runner) -> ! {
    runner.run().await
}

#[embassy_executor::task]
pub async fn net_task(mut runner: Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
pub async fn http_task(stack: embassy_net::Stack<'static>, seed: u64) {
    stack.wait_link_up().await;
    stack.wait_config_up().await;

    static TCP_STATE: StaticCell<TcpClientState<1, 4096, 4096>> = StaticCell::new();
    let client_state = TCP_STATE.init(TcpClientState::new());
    let tcp_client = TcpClient::new(stack, client_state);
    let dns_client = DnsSocket::new(stack);

    let mut rx_buffer = [0; 4096];
    let mut tls_read_buffer = [0; 16640];
    let mut write_buffer = [0; 16640];

    let tls_cfg = TlsConfig::new(
        seed,
        &mut tls_read_buffer,
        &mut write_buffer,
        reqwless::client::TlsVerify::None,
    );

    let mut https_client = HttpClient::new_with_tls(&tcp_client, &dns_client, tls_cfg);

    loop {
        let request = HTTP_CHANNEL.receive().await;

        match request {
            HttpRequest::PostSensorData(_data) => {}
            HttpRequest::SendAlert { message: _ } => {
                // TODO: POST alert
            }
            HttpRequest::Heartbeat => {
                // TODO: GET health check
            }
        }
    }
}
