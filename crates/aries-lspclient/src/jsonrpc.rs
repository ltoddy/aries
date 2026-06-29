#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    Str(String),
}

impl From<i64> for RequestId {
    fn from(id: i64) -> Self {
        Self::Number(id)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Request {
    pub fn new(id: impl Into<RequestId>, method: impl Into<String>, params: Value) -> Self {
        Self { id: id.into(), method: method.into(), params: Some(params) }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Notification {
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl Notification {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self { method: method.into(), params: Some(params) }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Response {
    Result { id: RequestId, result: Value },
    Error { id: RequestId, error: ResponseError },
}

impl Response {
    pub fn id(&self) -> &RequestId {
        match self {
            Self::Result { id, .. } | Self::Error { id, .. } => id,
        }
    }

    pub fn into_result(self) -> (RequestId, Result<Value, ResponseError>) {
        match self {
            Self::Result { id, result } => (id, Ok(result)),
            Self::Error { id, error } => (id, Err(error)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl std::fmt::Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum JsonRpcVersion {
    #[serde(rename = "2.0")]
    V2,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcMessage<M> {
    jsonrpc: JsonRpcVersion,
    #[serde(flatten)]
    pub message: M,
}

impl<M> JsonRpcMessage<M> {
    pub fn wrap(message: M) -> Self {
        Self { jsonrpc: JsonRpcVersion::V2, message }
    }
}
