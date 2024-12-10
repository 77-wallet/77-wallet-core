use std::time::Duration;

use rumqttc::v5::{mqttbytes::QoS, Client, Connection, MqttOptions};
use tokio::time;

pub struct MqttClient {
    client: Client,
}

impl MqttClient {
    // pub async fn new()
    pub fn client(&self) -> Client {
        self.client.clone()
    }

    pub fn publish(&self, topic: &str, qos: QoS, payload: &str) {
        self.client
            .publish(topic, qos, false, payload.as_bytes().to_vec())
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

    pub fn build(self) -> Result<(MqttClient, Connection), crate::errors::TransportError> {
        let url = format!("{}?client_id={}", self.url, self.client_id);
        let mut mqttoptions = MqttOptions::parse_url(url)?;
        let mut connect_props = rumqttc::v5::mqttbytes::v5::ConnectProperties::new();
        connect_props.session_expiry_interval = Some(2 * 60 * 60); // 2 days in seconds
        mqttoptions.set_credentials(self.username, self.password);
        mqttoptions.set_connect_properties(connect_props);
        mqttoptions.set_keep_alive(Duration::from_secs(10));
        mqttoptions.set_transport(rumqttc::Transport::Tcp);
        mqttoptions
            .set_user_properties(self.user_property)
            .set_connection_timeout(20)
            .set_clean_start(false)
            .set_manual_acks(true);

        let (client, eventloop) = Client::new(mqttoptions, 10);

        Ok((MqttClient { client }, eventloop))
    }
}

async fn _requests(client: Client) {
    client.subscribe("hello/world", QoS::AtMostOnce).unwrap();

    for i in 1..=10 {
        client
            .publish("hello/world", QoS::ExactlyOnce, false, vec![1; i])
            .unwrap();

        time::sleep(Duration::from_secs(1)).await;
    }

    time::sleep(Duration::from_secs(120)).await;
}
