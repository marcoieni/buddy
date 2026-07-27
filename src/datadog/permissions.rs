use anyhow::{Context, bail};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
pub(super) struct PermissionsSnapshot {
    permissions: Vec<Permission>,
}

#[derive(Debug, Serialize)]
struct Permission {
    id: String,
    name: Value,
    display_name: Value,
    description: Value,
    group_name: Value,
    name_aliases: Vec<String>,
    restricted: Value,
}

pub(super) fn normalize(current_user_json: &str) -> anyhow::Result<PermissionsSnapshot> {
    let response: Value = serde_json::from_str(current_user_json)
        .context("current_user did not return valid JSON")?;

    let included = response
        .get("included")
        .and_then(Value::as_array)
        .context("current_user did not return included permissions")?;

    let mut permissions = Vec::new();

    for item in included {
        if item.get("type").and_then(Value::as_str) == Some("permissions") {
            permissions.push(normalize_permission(item)?);
        }
    }

    permissions.sort_by(|left, right| left.id.cmp(&right.id));

    if permissions.is_empty() {
        bail!("current_user did not return any permissions");
    }

    Ok(PermissionsSnapshot { permissions })
}

fn normalize_permission(item: &Value) -> anyhow::Result<Permission> {
    let mut name_aliases = optional_string_array_at(item, "/attributes/name_aliases")
        .context("a Datadog permission returned invalid name aliases")?;
    name_aliases.sort();

    Ok(Permission {
        id: string_at(item, "/id").context("a Datadog permission has no ID")?,
        name: value_at(item, "/attributes/name"),
        display_name: value_at(item, "/attributes/display_name"),
        description: value_at(item, "/attributes/description"),
        group_name: value_at(item, "/attributes/group_name"),
        name_aliases,
        restricted: value_at(item, "/attributes/restricted"),
    })
}

fn string_at(value: &Value, pointer: &str) -> Option<String> {
    value.pointer(pointer)?.as_str().map(str::to_owned)
}

trait StringArrayAt {
    fn read(self, value: &Value) -> anyhow::Result<Vec<String>>;
}

impl StringArrayAt for &str {
    fn read(self, value: &Value) -> anyhow::Result<Vec<String>> {
        let array = value
            .pointer(self)
            .and_then(Value::as_array)
            .with_context(|| format!("expected an array at {self}"))?;
        array
            .iter()
            .map(|entry| {
                entry
                    .as_str()
                    .map(str::to_owned)
                    .with_context(|| format!("expected a string in {self}"))
            })
            .collect()
    }
}

fn string_array_at<A: StringArrayAt>(value: &Value, pointer: A) -> anyhow::Result<Vec<String>> {
    pointer.read(value)
}

fn optional_string_array_at(value: &Value, pointer: &str) -> anyhow::Result<Vec<String>> {
    match value.pointer(pointer) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(_) => string_array_at(value, pointer),
    }
}

fn value_at(value: &Value, pointer: &str) -> Value {
    value.pointer(pointer).cloned().unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURRENT_USER: &str = r#"
    {
      "data": {
        "id": "service-account-1",
        "attributes": {"name": "Buddy"}
      },
      "included": [
        {
          "type": "permissions",
          "id": "permission-b",
          "attributes": {
            "name": "b",
            "display_name": "B",
            "description": null,
            "group_name": "group",
            "name_aliases": ["z", "a"],
            "restricted": false
          }
        },
        {
          "type": "roles",
          "id": "role-1",
          "attributes": {
            "name": "Read only",
            "receives_permissions_from": []
          },
          "relationships": {
            "permissions": {
              "data": [{"id": "permission-b"}, {"id": "permission-a"}]
            }
          }
        },
        {
          "type": "permissions",
          "id": "permission-a",
          "attributes": {
            "name": "a",
            "display_name": "A",
            "description": "description",
            "group_name": "group",
            "restricted": true
          }
        }
      ]
    }
    "#;

    #[test]
    fn normalizes_and_sorts_permissions() {
        let snapshot = normalize(CURRENT_USER).unwrap();
        let json = serde_json::to_value(snapshot).unwrap();

        assert_eq!(
            json.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["permissions"]
        );
        assert_eq!(json["permissions"][0]["id"], "permission-a");
        assert_eq!(
            json["permissions"][0]["name_aliases"],
            serde_json::json!([])
        );
        assert_eq!(
            json["permissions"][1]["name_aliases"],
            serde_json::json!(["a", "z"])
        );
    }
}
