use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Default, Serialize)]
pub struct ToolList(Vec<String>);

impl ToolList {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

impl From<Vec<String>> for ToolList {
    fn from(value: Vec<String>) -> Self {
        Self(value)
    }
}

impl<'de> Deserialize<'de> for ToolList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ToolListValue {
            String(String),
            List(Vec<String>),
        }

        let value = Option::<ToolListValue>::deserialize(deserializer)?;
        Ok(Self(value.map_or_else(Vec::new, |value| {
            match value {
                ToolListValue::String(value) => value
                    .split([',', ' '])
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .collect(),
                ToolListValue::List(values) => values,
            }
        })))
    }
}
