use super::dev::*;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Schema<T> {
    pub paths: HashMap<String, T>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SchemaStorage<T> {
    pub name: String,
    pub schema: HashMap<String, HashMap<Os, Schema<T>>>,
}

impl<T> SchemaStorage<T> {
    pub fn search_compatible(&self, mut os: Os, name: &str) -> Option<&Schema<T>> {
        let repo = &self.schema.get(name)?;
        loop {
            if let Some(app) = repo.get(&os) {
                break Some(app);
            }
            if let Some(next_os) = os.next_compatible() {
                os = next_os;
            } else {
                break None;
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerdeSchemaStorage<T> {
    pub name: String,
    pub schema: HashMap<String, HashMap<String, Schema<T>>>,
}

impl<T> SerdeSchemaStorage<T> {
    pub fn into_storage(self) -> SchemaStorage<T> {
        let mut schemas = HashMap::new();
        for (name, all) in self.schema {
            let mut schema = HashMap::new();
            for (os, app) in all {
                let os = Os::from(os.as_str());
                schema.insert(os, app);
            }
            schemas.insert(name, schema);
        }
        SchemaStorage {
            name: self.name,
            schema: schemas,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_serialization() {
        let schema = r#"
        name = "default"

        [schema.fish.linux.paths]
        default = ["~/.config/fish"]
        "#;
        let storage: SerdeSchemaStorage<Vec<String>> = toml::from_str(schema).unwrap();
        let schema_storage = storage.into_storage();
        assert_eq!(schema_storage.name, "default");
        assert!(schema_storage.schema.contains_key("fish"));
        let app_schema = schema_storage.search_compatible(Os::linux(), "fish");
        assert!(app_schema.is_some());
        let app_schema = app_schema.unwrap();
        assert!(app_schema.paths.contains_key("default"));
        assert_eq!(app_schema.paths["default"], vec!["~/.config/fish"]);
    }
}
