//! Author-facing Discord Rich Presence configuration and activity.

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordConfig {
    #[serde(deserialize_with = "application_id")]
    pub application_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DiscordActivity {
    #[serde(deserialize_with = "text")]
    pub details: Option<String>,
    #[serde(deserialize_with = "text")]
    pub state: Option<String>,
    #[serde(deserialize_with = "image")]
    pub large_image: Option<String>,
    #[serde(deserialize_with = "text")]
    pub large_text: Option<String>,
    #[serde(deserialize_with = "image")]
    pub small_image: Option<String>,
    #[serde(deserialize_with = "text")]
    pub small_text: Option<String>,
    /// Unix timestamp in seconds, rather than JavaScript milliseconds.
    #[serde(deserialize_with = "timestamp")]
    pub start_timestamp: Option<u64>,
}

fn application_id<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let value = String::deserialize(d)?;
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || value.parse::<u64>().ok().is_none_or(|id| id == 0)
    {
        return Err(serde::de::Error::custom(
            "Discord applicationId must be a nonzero decimal u64 string",
        ));
    }
    Ok(value)
}

fn bounded_string<'de, D: Deserializer<'de>>(
    d: D,
    min: usize,
    max: usize,
) -> Result<Option<String>, D::Error> {
    let value = Option::<String>::deserialize(d)?;
    if value
        .as_ref()
        .is_some_and(|value| !(min..=max).contains(&value.len()))
    {
        return Err(serde::de::Error::custom(format!(
            "Discord activity string must contain {min}..={max} UTF-8 bytes",
        )));
    }
    Ok(value)
}

fn text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    bounded_string(d, 2, 128)
}

fn image<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    bounded_string(d, 1, 256)
}

fn timestamp<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    let value = Option::<u64>::deserialize(d)?;
    if value.is_some_and(|value| value > i64::MAX.cast_unsigned()) {
        return Err(serde::de::Error::custom(
            "Discord startTimestamp must be an integer between 0 and i64::MAX",
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn application_ids_preserve_integer_precision_and_reject_invalid_values() {
        let config: DiscordConfig = serde_json::from_value(json!({
            "applicationId": "1234567890123456789",
        }))
        .unwrap();
        assert_eq!(config.application_id, "1234567890123456789");
        for invalid in [
            json!(0),
            json!("0"),
            json!(""),
            json!("-1"),
            json!("+1"),
            json!("1.5"),
            json!(" 1"),
            json!("18446744073709551616"),
        ] {
            assert!(
                serde_json::from_value::<DiscordConfig>(json!({
                    "applicationId": invalid,
                }))
                .is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn activity_validates_utf8_byte_limits_and_integer_timestamps() {
        assert_eq!(
            serde_json::from_value::<DiscordActivity>(json!({})).unwrap(),
            DiscordActivity::default()
        );
        let valid: DiscordActivity = serde_json::from_value(json!({
            "details": "é".repeat(64), "largeImage": "x".repeat(256),
            "startTimestamp": i64::MAX,
        }))
        .unwrap();
        assert_eq!(valid.details.unwrap().len(), 128);
        for invalid in [
            json!({"details": "a"}),
            json!({"state": "é".repeat(65)}),
            json!({"largeImage": ""}),
            json!({"smallImage": "x".repeat(257)}),
            json!({"startTimestamp": -1}),
            json!({"startTimestamp": 1.5}),
            json!({"startTimestamp": u64::MAX}),
        ] {
            assert!(
                serde_json::from_value::<DiscordActivity>(invalid.clone()).is_err(),
                "accepted {invalid}"
            );
        }
    }
}
