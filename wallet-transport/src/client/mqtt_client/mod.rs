pub mod sync;
use std::time::Duration;

use rumqttc::v5::{mqttbytes::QoS, AsyncClient, EventLoop, MqttOptions};
use tokio::time;

pub struct MqttAsyncClient {
    client: AsyncClient,
}

impl MqttAsyncClient {
    // pub async fn new()
    pub fn client(&self) -> AsyncClient {
        self.client.clone()
    }

    pub async fn publish(&self, topic: &str, qos: QoS, payload: &str) {
        self.client
            .publish(topic, qos, false, payload.as_bytes().to_vec())
            .await
            .unwrap();
    }
}
pub struct MqttClientBuilder {
    client_id: String,
    url: String,
    username: String,
    password: String,
    user_property: Vec<(String, String)>,
}

impl MqttClientBuilder {
    pub fn new(
        client_id: &str,
        url: &str,
        username: &str,
        password: &str,
        user_property: Vec<(String, String)>,
    ) -> Self {
        Self {
            client_id: client_id.to_string(),
            url: url.to_string(),
            user_property,
            username: username.to_string(),
            password: password.to_string(),
        }
    }

    pub fn build(self) -> Result<(MqttAsyncClient, EventLoop), crate::errors::TransportError> {
        let url = format!("{}?client_id={}", self.url, self.client_id);
        let mut mqttoptions = MqttOptions::parse_url(url)?;

        mqttoptions
            // .set_connect_properties(connect_props)
            .set_transport(rumqttc::Transport::Tcp)
            .set_connection_timeout(20)
            .set_credentials(self.username, self.password)
            .set_keep_alive(Duration::from_secs(10))
            .set_user_properties(self.user_property)
            .set_clean_start(true)
            .set_manual_acks(true);

        let (client, eventloop) = AsyncClient::new(mqttoptions, 10);

        Ok((MqttAsyncClient { client }, eventloop))
    }
}

// 测试用
pub async fn _mqtt_connect(username: &str, password: &str, content: &str, client_id: &str) {
    let user_property = vec![
        ("content".to_string(), content.to_string()),
        ("clientId".to_string(), client_id.to_string()),
    ];

    let url = format!("{}?client_id={}", "mqtt-endpoint", client_id);
    let mut mqttoptions = MqttOptions::parse_url(url).unwrap();
    let mut connect_props = rumqttc::v5::mqttbytes::v5::ConnectProperties::new();
    connect_props.session_expiry_interval = Some(2 * 60 * 60); // 2 days in seconds

    mqttoptions
        .set_connect_properties(connect_props)
        .set_transport(rumqttc::Transport::Tcp)
        .set_credentials(username, password)
        .set_keep_alive(Duration::from_secs(10))
        .set_user_properties(user_property)
        .set_clean_start(false)
        .set_manual_acks(true);

    let (_client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    while let Ok(event) = eventloop.poll().await {
        if let rumqttc::v5::Event::Incoming(packet) = event {
            let _publish = match packet {
                rumqttc::v5::mqttbytes::v5::Packet::Publish(publish) => publish,
                _ => continue,
            };
        }
    }
}

async fn _requests(client: AsyncClient) {
    client
        .subscribe("hello/world", QoS::AtMostOnce)
        .await
        .unwrap();

    for i in 1..=10 {
        client
            .publish("hello/world", QoS::ExactlyOnce, false, vec![1; i])
            .await
            .unwrap();

        time::sleep(Duration::from_secs(1)).await;
    }

    time::sleep(Duration::from_secs(120)).await;
}
