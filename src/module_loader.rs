use std::{collections::HashMap, fs, path::PathBuf, sync::Mutex};

use super::ExecutionError;

struct Inner {
    paths: Vec<PathBuf>,
    embedded: HashMap<PathBuf, String>,
}

pub struct ModuleLoader {
    inner: Mutex<Inner>,
}

impl ModuleLoader {
    pub fn new() -> Self {
        let inner = Inner {
            paths: Vec::new(),
            embedded: HashMap::new(),
        };
        Self {
            inner: Mutex::new(inner),
        }
    }

    pub fn add_embedded(&self, embedded: HashMap<String, String>) {
        let mut inner = self.inner.lock().unwrap();
        for (path, code) in embedded {
            inner.embedded.insert(path.into(), code);
        }
    }

    pub fn add_paths(&self, paths: Vec<PathBuf>) {
        self.inner.lock().unwrap().paths.extend(paths);
    }

    pub fn load(&self, module: &str) -> Result<String, ExecutionError> {
        let inner = self.inner.lock().unwrap();
        if !inner.embedded.is_empty() {
            let mut path = PathBuf::from(module);
            if let Some(code) = inner.embedded.get(&path) {
                return Ok(code.clone());
            }
            if path.extension().is_none() {
                path.set_extension("js");
                if let Some(code) = inner.embedded.get(&path) {
                    return Ok(code.clone());
                }
                path.set_extension("mjs");
                if let Some(code) = inner.embedded.get(&path) {
                    return Ok(code.clone());
                }
            }
        }
        for dir in &inner.paths {
            let mut path = dir.join(module);
            if let Ok(code) = fs::read_to_string(&path) {
                return Ok(code.clone());
            }
            if path.extension().is_none() {
                path.set_extension("js");
                if let Ok(code) = fs::read_to_string(&path) {
                    return Ok(code.clone());
                }
                path.set_extension("mjs");
                if let Ok(code) = fs::read_to_string(&path) {
                    return Ok(code.clone());
                }
            }
        }

        Err(ExecutionError::ModuleNotFound {
            name: module.to_string(),
        })
    }
}
