//! Logic shared by more than one of the JVM subcommands.

use serde::Serialize;
use serde_json::Value;

/// Serializes a map with its keys in a fixed order (serde_json's map sorts).
pub struct OrderedObject<'a>(pub serde_json::Map<String, Value>, pub &'a [&'a str]);

impl Serialize for OrderedObject<'_> {
  fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    let mut map = s.serialize_map(None)?;
    for key in self.1 {
      if let Some(v) = self.0.get(*key) {
        map.serialize_entry(key, v)?;
      }
    }
    map.end()
  }
}
