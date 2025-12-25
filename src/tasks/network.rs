use cyw43_pio::PioSpi;
use embassy_net::Runner;
use embassy_net::dns::DnsSocket;
use embassy_net::tcp::client::{TcpClient, TcpClientState};
use embassy_rp::gpio::Output;
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_time::{Duration, Timer};
use heapless::String;
use log::{error, info};
use reqwless::client::{HttpClient, TlsConfig, TlsVerify};
use reqwless::headers::ContentType;
use reqwless::request::{Method, RequestBuilder};
use static_cell::StaticCell;

use crate::channels::HTTP_CHANNEL;
use crate::config::{API_KEY, POLL_INTERVAL_SECS, SENSOR_ENDPOINT, SERVER_URL, TASKS_ENDPOINT};
use crate::types::HttpRequest;

pub type Cyw43Runner = cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>;

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
    info!("HTTP task: network ready");

    static TCP_STATE: StaticCell<TcpClientState<1, 4096, 4096>> = StaticCell::new();
    let client_state = TCP_STATE.init(TcpClientState::new());
    let tcp_client = TcpClient::new(stack, client_state);
    let dns_client = DnsSocket::new(stack);

    loop {
        let request = HTTP_CHANNEL.receive().await;

        // Buffers for this request
        let mut rx_buffer = [0; 4096];
        let mut tls_read_buffer = [0; 16640];
        let mut tls_write_buffer = [0; 16640];

        let tls_cfg = TlsConfig::new(
            seed,
            &mut tls_read_buffer,
            &mut tls_write_buffer,
            TlsVerify::None,
        );

        let mut https_client = HttpClient::new_with_tls(&tcp_client, &dns_client, tls_cfg);

        let mut url: String<128> = String::new();
        let _ = url.push_str(SERVER_URL);

        match request {
            HttpRequest::PostSensorData(data) => {
                let _ = url.push_str(SENSOR_ENDPOINT);

                // Serialize sensor data
                let mut body_buffer = [0u8; 256];
                let len = match serde_json_core::to_slice(&data, &mut body_buffer) {
                    Ok(len) => len,
                    Err(e) => {
                        error!("Failed to serialize sensor data: {:?}", e);
                        continue;
                    }
                };

                info!("POST {} ({} bytes)", url.as_str(), len);

                let req = match https_client.request(Method::POST, url.as_str()).await {
                    Ok(req) => req,
                    Err(e) => {
                        error!("Request failed: {:?}", e);
                        continue;
                    }
                };

                match req
                    .body(&body_buffer[..len])
                    .content_type(ContentType::ApplicationJson)
                    .headers(&[("X-Api-Key", API_KEY)])
                    .send(&mut rx_buffer)
                    .await
                {
                    Ok(response) => {
                        info!("Response: {}", response.status.0);
                    }
                    Err(e) => {
                        error!("POST failed: {:?}", e);
                    }
                }
            }

            HttpRequest::SendAlert { message } => {
                let _ = url.push_str("/alert");

                info!("POST alert: {}", message.as_str());

                let mut body_buffer = [0u8; 128];
                let len = match serde_json_core::to_slice(&message, &mut body_buffer) {
                    Ok(len) => len,
                    Err(_) => continue,
                };

                let req = match https_client.request(Method::POST, url.as_str()).await {
                    Ok(req) => req,
                    Err(e) => {
                        error!("Alert request failed: {:?}", e);
                        continue;
                    }
                };

                if let Err(e) = req
                    .body(&body_buffer[..len])
                    .content_type(ContentType::ApplicationJson)
                    .headers(&[("X-Api-Key", API_KEY)])
                    .send(&mut rx_buffer)
                    .await
                {
                    error!("Alert POST failed: {:?}", e);
                }
            }

            HttpRequest::PollTasks => {
                let _ = url.push_str(TASKS_ENDPOINT);

                let req = match https_client.request(Method::GET, url.as_str()).await {
                    Ok(req) => req,
                    Err(e) => {
                        error!("Poll request failed: {:?}", e);
                        continue;
                    }
                };

                match req
                    .headers(&[("X-Api-Key", API_KEY)])
                    .send(&mut rx_buffer)
                    .await
                {
                    Ok(response) => {
                        info!("Tasks response: {}", response.status.0);
                        // TODO: Parse response body and dispatch tasks
                        let _ = response.body().read_to_end().await;
                    }
                    Err(e) => {
                        error!("Poll tasks failed: {:?}", e);
                    }
                }
            }
        }
    }
}

#[embassy_executor::task]
pub async fn poll_task() {
    Timer::after(Duration::from_secs(5)).await;

    loop {
        HTTP_CHANNEL.send(HttpRequest::PollTasks).await;
        Timer::after(Duration::from_secs(POLL_INTERVAL_SECS)).await;
    }
}
