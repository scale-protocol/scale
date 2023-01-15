use axum::Json;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonResponse<T> {
    message: String,
    data: Option<T>,
}
impl<T> Default for JsonResponse<T> {
    fn default() -> Self {
        Self {
            message: "Ok".to_string(),
            data: None,
        }
    }
}
impl<T> From<Option<T>> for JsonResponse<T> {
    fn from(s: Option<T>) -> Self {
        Self {
            message: "Ok".to_string(),
            data: s,
        }
    }
}

impl<T> From<anyhow::Result<T>> for JsonResponse<T> {
    fn from(s: anyhow::Result<T>) -> Self {
        match s {
            Ok(r) => Self {
                message: "Ok".to_string(),
                data: Some(r),
            },
            Err(e) => Self {
                message: e.to_string(),
                data: None,
            },
        }
    }
}
pub struct JsonOk(pub [u8]);
impl<T> JsonResponse<T> {
    pub fn new(r: T) -> Self {
        Self {
            message: "Ok".to_string(),
            data: Some(r),
        }
    }
    pub fn to_json(self) -> Json<Self> {
        Json(self)
    }
    pub fn err(mut self, e: anyhow::Error) -> Self {
        self.message = e.to_string();
        self
    }
}
