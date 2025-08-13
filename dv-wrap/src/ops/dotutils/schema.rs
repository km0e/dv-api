use super::dev::*;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AppSchema {
    pub paths: HashMap<String, Vec<String>>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SourceSchema {
    pub paths: HashMap<String, String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SchemaStorage {
    pub name: String,
    pub schema: HashMap<String, HashMap<Os, AppSchema>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SourceStorage {
    pub name: String,
    pub schema: HashMap<String, HashMap<Os, SourceSchema>>,
}

impl SchemaStorage {
    pub fn search_compatible(&self, mut os: Os, name: &str) -> Option<&AppSchema> {
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

impl SourceStorage {
    pub fn search_compatible(&self, mut os: Os, name: &str) -> Option<&SourceSchema> {
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
pub struct SerdeSchemaStorage {
    pub name: String,
    pub schema: HashMap<String, HashMap<String, AppSchema>>,
}

impl SerdeSchemaStorage {
    pub fn into_schema_storage(self) -> SchemaStorage {
        let mut schemas = HashMap::new();
        for (name, all) in self.schema {
            let mut schema = HashMap::new();
            for (os, app) in all {
                let os = Os::from(os.as_str());
                schema.insert(os, app.clone());
            }
            schemas.insert(name, schema);
        }
        SchemaStorage {
            name: self.name,
            schema: schemas,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerdeSourceStorage {
    pub name: String,
    pub schema: HashMap<String, HashMap<String, SourceSchema>>,
}

impl SerdeSourceStorage {
    pub fn into_source_storage(self) -> SourceStorage {
        let mut schemas = HashMap::new();
        for (name, all) in self.schema {
            let mut schema = HashMap::new();
            for (os, app) in all {
                let os = Os::from(os.as_str());
                schema.insert(os, app.clone());
            }
            schemas.insert(name, schema);
        }
        SourceStorage {
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

        [schemas.fish.linux.paths]
        default = ["~/.config/fish"]
        "#;
        let storage: SerdeSchemaStorage = toml::from_str(schema).unwrap();
        let schema_storage = storage.into_schema_storage();
        assert_eq!(schema_storage.name, "default");
        assert!(schema_storage.schema.contains_key("fish"));
        let app_schema = schema_storage.search_compatible(Os::linux(), "fish");
        assert!(app_schema.is_some());
        let app_schema = app_schema.unwrap();
        assert!(app_schema.paths.contains_key("default"));
        assert_eq!(app_schema.paths["default"], vec!["~/.config/fish"]);
    }
}
