use std::io::{self, Write};

use serde_json::Value;

use crate::{inventory_to_json, workflow::OperationOutcome};

pub fn outcome_to_value(outcome: &OperationOutcome) -> Result<Value, serde_json::Error> {
    match outcome {
        OperationOutcome::Inventory(inventory) => {
            serde_json::to_value(inventory_to_json(inventory))
        }
        OperationOutcome::Import(import) => serde_json::to_value(import),
        OperationOutcome::RepositoryImport(result) => serde_json::to_value(result),
        OperationOutcome::SkillOperation(result) => serde_json::to_value(result),
    }
}

pub fn write_outcome(
    mut writer: impl Write,
    outcome: &OperationOutcome,
) -> Result<(), JsonWriteError> {
    match outcome {
        OperationOutcome::Inventory(inventory) => {
            serde_json::to_writer_pretty(&mut writer, &inventory_to_json(inventory))?;
        }
        OperationOutcome::Import(import) => {
            serde_json::to_writer_pretty(&mut writer, import)?;
        }
        OperationOutcome::RepositoryImport(result) => {
            serde_json::to_writer_pretty(&mut writer, result)?;
        }
        OperationOutcome::SkillOperation(result) => {
            serde_json::to_writer_pretty(&mut writer, result)?;
        }
    }
    writeln!(writer)?;
    Ok(())
}

#[derive(Debug)]
pub enum JsonWriteError {
    Serialize(serde_json::Error),
    Io(io::Error),
}

impl std::fmt::Display for JsonWriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialize(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for JsonWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Serialize(error) => Some(error),
            Self::Io(error) => Some(error),
        }
    }
}

impl From<serde_json::Error> for JsonWriteError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialize(error)
    }
}

impl From<io::Error> for JsonWriteError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
