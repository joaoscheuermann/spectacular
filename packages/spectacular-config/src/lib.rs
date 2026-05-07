use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fmt::{self, Display};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

const APP_CONFIG_DIR_NAME: &str = "spectacular";
const CONFIG_FILE_NAME: &str = "config.json";
const MODEL_CACHE_FILE_NAME: &str = "model-cache.json";
const SCHEMA_CHANGED_MESSAGE: &str =
    "configuration schema changed; reconfigure providers, models, and tasks";
const BACKUP_EXTENSION: &str = "bak.json";

include!("schema.rs");
include!("persistence.rs");
include!("config_model.rs");
include!("errors.rs");

#[cfg(test)]
mod tests {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/unit/config.rs"));
}
