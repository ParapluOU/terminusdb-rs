use crate::json::InstancePropertyFromJson;
use crate::{
    FromInstanceProperty, InstanceProperty, MaybeFromTDBInstance, Primitive, PrimitiveValue,
    Schema, ToInstanceProperty, ToMaybeTDBSchema, ToSchemaClass, DATE, DATETIME, TIME,
};
use anyhow::bail;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde_json::Value;

// Implement ToSchemaClass for DateTime<Utc>
impl ToSchemaClass for DateTime<Utc> {
    fn to_class() -> String {
        DATETIME.to_string()
    }
}

impl MaybeFromTDBInstance for DateTime<Utc> {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }

    fn maybe_from_property(prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        if let InstanceProperty::Primitive(PrimitiveValue::String(s)) = prop {
            Ok(Some(s.parse()?))
        } else {
            Ok(None)
        }
    }
}

// Mark DateTime<Utc> as a primitive type
impl Primitive for DateTime<Utc> {}

// Implement ToMaybeTDBSchema for DateTime<Utc> (default impl is fine)
impl ToMaybeTDBSchema for DateTime<Utc> {}

// Implement conversion from DateTime<Utc> to PrimitiveValue
impl From<DateTime<Utc>> for PrimitiveValue {
    fn from(dt: DateTime<Utc>) -> Self {
        Self::String(dt.to_rfc3339())
    }
}

// Implement conversion from DateTime<Utc> to InstanceProperty
impl From<DateTime<Utc>> for InstanceProperty {
    fn from(dt: DateTime<Utc>) -> Self {
        Self::Primitive(dt.into())
    }
}

// Implement ToInstanceProperty for DateTime<Utc>
impl<Parent> ToInstanceProperty<Parent> for DateTime<Utc> {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

impl FromInstanceProperty for DateTime<Utc> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::String(s)) = prop {
            Ok(s.parse()?)
        } else {
            Err(anyhow::anyhow!("Expected String primitive value"))
        }
    }
}

// todo: validate?
impl<Parent> InstancePropertyFromJson<Parent> for DateTime<Utc> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        match json.as_str() {
            Some(s) => Ok(InstanceProperty::Primitive(PrimitiveValue::String(
                s.to_string(),
            ))),
            None => bail!("Expected a string, got: {}", json),
        }
    }
}

// Implement ToSchemaClass for NaiveTime
impl ToSchemaClass for NaiveTime {
    fn to_class() -> String {
        TIME.to_string()
    }
}

// Mark NaiveTime as a primitive type
impl Primitive for NaiveTime {}

// Implement ToMaybeTDBSchema for NaiveTime (default impl is fine)
impl ToMaybeTDBSchema for NaiveTime {}

// Implement conversion from NaiveTime to PrimitiveValue
impl From<NaiveTime> for PrimitiveValue {
    fn from(time: NaiveTime) -> Self {
        Self::String(time.format("%H:%M:%S%.f").to_string())
    }
}

// Implement conversion from NaiveTime to InstanceProperty
impl From<NaiveTime> for InstanceProperty {
    fn from(time: NaiveTime) -> Self {
        Self::Primitive(time.into())
    }
}

// Implement ToInstanceProperty for NaiveTime
impl<Parent> ToInstanceProperty<Parent> for NaiveTime {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

/// Strip a trailing timezone designator (`Z`, or a `+hh:mm` / `-hh:mm` offset)
/// from an xsd:time lexical form, leaving the bare time-of-day for `NaiveTime`.
fn strip_time_zone(s: &str) -> &str {
    if let Some(stripped) = s.strip_suffix('Z') {
        return stripped;
    }
    // A pure time string has no date, so a trailing '+'/'-' can only start an
    // offset. Only strip if the head still looks like a time (contains ':').
    if let Some(pos) = s.rfind(['+', '-']) {
        let (head, _offset) = s.split_at(pos);
        if head.contains(':') {
            return head;
        }
    }
    s
}

impl FromInstanceProperty for NaiveTime {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::String(s)) = prop {
            // TerminusDB stores xsd:time with a timezone, so a value round-trips
            // back as e.g. "15:09:26Z" (or with a "+hh:mm" offset). NaiveTime has
            // no timezone, so strip a trailing "Z" / offset before parsing.
            let s = strip_time_zone(s);
            // Try parsing with different formats
            if let Ok(time) = NaiveTime::parse_from_str(s, "%H:%M:%S%.f") {
                Ok(time)
            } else if let Ok(time) = NaiveTime::parse_from_str(s, "%H:%M:%S") {
                Ok(time)
            } else if let Ok(time) = NaiveTime::parse_from_str(s, "%H:%M") {
                Ok(time)
            } else {
                Err(anyhow::anyhow!("Failed to parse time string: {}", s))
            }
        } else {
            Err(anyhow::anyhow!("Expected String primitive value"))
        }
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for NaiveTime {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        match json.as_str() {
            Some(s) => Ok(InstanceProperty::Primitive(PrimitiveValue::String(
                s.to_string(),
            ))),
            None => bail!("Expected a string, got: {}", json),
        }
    }
}

// === chrono::NaiveDate as xsd:date ===

impl ToSchemaClass for NaiveDate {
    fn to_class() -> String {
        DATE.to_string()
    }
}

impl Primitive for NaiveDate {}

impl ToMaybeTDBSchema for NaiveDate {}

impl From<NaiveDate> for PrimitiveValue {
    fn from(date: NaiveDate) -> Self {
        Self::String(date.format("%Y-%m-%d").to_string())
    }
}

impl From<NaiveDate> for InstanceProperty {
    fn from(date: NaiveDate) -> Self {
        Self::Primitive(date.into())
    }
}

impl<Parent> ToInstanceProperty<Parent> for NaiveDate {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        self.into()
    }
}

/// Reduce an xsd:date lexical form to the bare `YYYY-MM-DD` date. TerminusDB may
/// round-trip a date with a timezone ("2021-03-14Z", "2021-03-14+01:00") or as a
/// dateTime ("2021-03-14T00:00:00Z"); `NaiveDate` carries none of that.
fn strip_date_zone(s: &str) -> &str {
    let s = s.split('T').next().unwrap_or(s);
    let s = s.strip_suffix('Z').unwrap_or(s);
    // A date is 10 chars (YYYY-MM-DD); a trailing +hh:mm / -hh:mm offset follows.
    if s.len() > 10 {
        if let Some(rel) = s[10..].find(['+', '-']) {
            return &s[..10 + rel];
        }
    }
    s
}

impl FromInstanceProperty for NaiveDate {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        if let InstanceProperty::Primitive(PrimitiveValue::String(s)) = prop {
            let s = strip_date_zone(s);
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| anyhow::anyhow!("Failed to parse date string: {}", s))
        } else {
            Err(anyhow::anyhow!("Expected String primitive value"))
        }
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for NaiveDate {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        match json.as_str() {
            Some(s) => Ok(InstanceProperty::Primitive(PrimitiveValue::String(
                s.to_string(),
            ))),
            None => bail!("Expected a string, got: {}", json),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use crate::ToSchemaProperty;
    use chrono::Timelike;

    #[test]
    fn test_datetime_schema_property() {
        let property = <DateTime<Utc> as ToSchemaProperty<()>>::to_schema_property("created_at");
        assert_eq!(property.name, "created_at");
        assert_eq!(property.class, DATETIME);
    }

    #[test]
    fn test_datetime_instance_property() {
        let now = Utc::now();
        let property = <chrono::DateTime<chrono::Utc> as ToInstanceProperty<()>>::to_property(
            now,
            "created_at",
            &Schema::empty_class("Test"),
        );
        match property {
            InstanceProperty::Primitive(PrimitiveValue::String(s)) => {
                assert_eq!(s, now.to_rfc3339());
            }
            _ => panic!("Expected String primitive value"),
        }
    }

    #[test]
    fn test_naive_time_schema_property() {
        let property = <NaiveTime as ToSchemaProperty<()>>::to_schema_property("start_time");
        assert_eq!(property.name, "start_time");
        assert_eq!(property.class, TIME);
    }

    #[test]
    fn test_naive_time_instance_property() {
        let time = NaiveTime::from_hms_opt(14, 30, 45).unwrap();
        let property = <chrono::NaiveTime as ToInstanceProperty<()>>::to_property(
            time,
            "start_time",
            &Schema::empty_class("Test"),
        );
        match property {
            InstanceProperty::Primitive(PrimitiveValue::String(s)) => {
                assert_eq!(s, "14:30:45");
            }
            _ => panic!("Expected String primitive value"),
        }
    }

    #[test]
    fn test_naive_time_from_instance_property() {
        let time_str = "14:30:45.123";
        let instance_prop =
            InstanceProperty::Primitive(PrimitiveValue::String(time_str.to_string()));

        let result = NaiveTime::from_property(&instance_prop);
        assert!(result.is_ok());

        let time = result.unwrap();
        assert_eq!(time.hour(), 14);
        assert_eq!(time.minute(), 30);
        assert_eq!(time.second(), 45);
        // Check that the time was parsed correctly by formatting it back
        let formatted = time.format("%H:%M:%S%.f").to_string();
        assert!(formatted.starts_with("14:30:45.123"));
    }
}
