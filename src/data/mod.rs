use serde::Deserialize;

pub mod pixiv;
pub mod fanbox;

fn de_str_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    s.parse::<u64>().map_err(serde::de::Error::custom)
}

fn de_str_or_u64_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StrOrU64;

    impl<'de> serde::de::Visitor<'de> for StrOrU64 {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("string or u64")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            v.parse::<u64>().map_err(serde::de::Error::custom)
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v)
        }
    }

    deserializer.deserialize_any(StrOrU64)
}

#[derive(Deserialize)]
pub struct Response<T> {
    pub error: bool,
    pub message: String,
    pub body: T,
}

impl<T> Response<T> {
    pub fn into_body(self) -> anyhow::Result<T> {
        if self.error {
            Err(anyhow::anyhow!("API error: {}", self.message))
        } else {
            Ok(self.body)
        }
    }
}
