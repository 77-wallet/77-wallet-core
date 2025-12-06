use crate::{TransportError, errors::NodeResponseError, types::JsonRpcResult};
use reqwest::RequestBuilder;
use serde::{Serialize, de::DeserializeOwned};
use std::{fmt::Debug, str::FromStr};

pub struct ReqBuilder(pub RequestBuilder);

impl ReqBuilder {
    pub fn json(mut self, v: impl Serialize + Debug) -> Self {
        tracing::debug!("request params: {}", serde_json::to_string(&v).unwrap());
        self.0 = self.0.json(&v);
        self
    }

    pub fn query(mut self, v: impl Serialize + Debug) -> Self {
        tracing::debug!("request params: {:?}", v);
        self.0 = self.0.query(&v);
        self
    }

    pub fn body(mut self, body: String) -> Self {
        tracing::debug!("request params: {:?}", body);
        self.0 = self.0.body(body);
        self
    }

    pub async fn do_request(self) -> Result<String, crate::TransportError> {
        // 在调用send()前获取rpc_url
        let rpc_url = get_rpc_url(self.0.try_clone());
        let res = self
            .0
            .send()
            .await
            .map_err(|e| TransportError::Utils(wallet_utils::Error::Http(e.into())))?;

        let status = res.status();
        if !status.is_success() {
            // 尝试解析出 json response:: btc now node 返回的不标准。
            match res.text().await {
                Ok(response) => {
                    // tracing::info!("response = {}", response);
                    if let Ok(rs) = try_to_paras_json(&response) {
                        return Err(TransportError::NodeResponseError(NodeResponseError::new(
                            rs.0,
                            &rpc_url,
                            Some(rs.1),
                        )));
                    } else {
                        // 尝试提出error
                        let message = match serde_json::Value::from_str(&response) {
                            Ok(value) => {
                                if let Some(e) = value.get("error") {
                                    Some(e.to_string())
                                } else {
                                    Some(format!("response no error :{}", response))
                                }
                            }
                            Err(_) => Some(format!("all response:{}", response)),
                        };
                        return Err(TransportError::NodeResponseError(NodeResponseError::new(
                            status.as_u16() as i64,
                            &rpc_url,
                            message,
                        )));
                    }
                }
                Err(e) => {
                    return Err(TransportError::NodeResponseError(NodeResponseError::new(
                        status.as_u16() as i64,
                        &rpc_url,
                        Some(format!("res to test err:{}", e)),
                    )));
                }
            }
        }

        let response = res
            .text()
            .await
            .map_err(|e| crate::TransportError::Utils(wallet_utils::Error::Http(e.into())))?;

        tracing::debug!("response = {}", response);
        Ok(response)
    }

    // 普通请求
    pub async fn send<T: DeserializeOwned>(self) -> Result<T, crate::TransportError> {
        let res = self.do_request().await?;
        Ok(wallet_utils::serde_func::serde_from_str(&res)?)
    }
}

/// 从RequestBuilder获取RPC URL，如果获取失败则返回"unknown"
fn get_rpc_url(builder: Option<RequestBuilder>) -> String {
    builder
        .and_then(|b| b.build().ok().map(|req| req.url().to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

fn try_to_paras_json(res: &str) -> Result<(i64, String), crate::TransportError> {
    // 尝试直接解析整个响应为JsonRpcResult
    match wallet_utils::serde_func::serde_from_str::<JsonRpcResult>(res) {
        Ok(result) => {
            if let Some(err) = result.error {
                return Ok((err.code, err.message));
            }
        }
        Err(_) => {
            // 如果直接解析失败，尝试使用正则表达式提取JSON部分（针对不标准的响应）
            if let Ok(reg) = regex::Regex::new(r#"\{.*\}"#) {
                if let Some(captured) = reg.find(res) {
                    let json_str = captured.as_str().replace("\\\"", "\"");
                    if let Ok(result) =
                        wallet_utils::serde_func::serde_from_str::<JsonRpcResult>(&json_str)
                    {
                        if let Some(err) = result.error {
                            return Ok((err.code, err.message));
                        }
                    }
                }
            }
        }
    }
    Err(crate::TransportError::EmptyResult)
}
