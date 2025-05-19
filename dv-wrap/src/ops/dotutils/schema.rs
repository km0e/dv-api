use super::dev::*;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AppSchema {
    pub paths: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerdeSchema {
    pub repo: HashMap<String, AppSchema>,
}

impl SerdeSchema {
    fn into_schema(self) -> Schema {
        let mut repo = HashMap::new();
        for (os, app) in self.repo {
            let os = Os::from(os.as_str());
            repo.insert(os, app.clone());
        }
        Schema { repo }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Schema {
    pub repo: HashMap<Os, AppSchema>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SchemaStorage {
    pub name: String,
    pub schemas: HashMap<String, Schema>,
}

impl SchemaStorage {
    pub fn search_compatible(&self, mut os: Os, name: &str) -> Option<&AppSchema> {
        let repo = &self.schemas.get(name)?.repo;
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
    pub schemas: HashMap<String, SerdeSchema>,
}

impl SerdeSchemaStorage {
    pub fn into_schema_storage(self) -> SchemaStorage {
        let mut schemas = HashMap::new();
        for (name, schema) in self.schemas {
            let schema = schema.into_schema();
            schemas.insert(name, schema);
        }
        SchemaStorage {
            name: self.name,
            schemas,
        }
    }
}
