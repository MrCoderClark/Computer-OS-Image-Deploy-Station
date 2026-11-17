use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSequence {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub steps: Vec<TaskStep>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStep {
    pub id: Uuid,
    pub step_type: StepType,
    pub name: String,
    pub enabled: bool,
    pub order: u32,
    pub config: StepConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Partition,
    ApplyImage,
    InjectDrivers,
    RunScript,
    DomainJoin,
    GenerateUnattend,
    Reboot,
}

impl std::fmt::Display for StepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Partition => write!(f, "Partition Disk"),
            Self::ApplyImage => write!(f, "Apply Image"),
            Self::InjectDrivers => write!(f, "Inject Drivers"),
            Self::RunScript => write!(f, "Run Script"),
            Self::DomainJoin => write!(f, "Domain Join"),
            Self::GenerateUnattend => write!(f, "Generate Unattend.xml"),
            Self::Reboot => write!(f, "Reboot"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepConfig {
    Partition {
        #[serde(default = "default_esp_size")]
        esp_size_mb: u32,
        #[serde(default = "default_msr_size")]
        msr_size_mb: u32,
    },
    ApplyImage {
        image_index: u32,
    },
    InjectDrivers {
        driver_pack_id: Option<Uuid>,
        driver_path: Option<String>,
    },
    RunScript {
        script_path: String,
        arguments: Vec<String>,
        #[serde(default)]
        run_elevated: bool,
    },
    DomainJoin {
        domain_name: String,
        ou_path: Option<String>,
        account: String,
    },
    GenerateUnattend {
        computer_name: Option<String>,
        locale: Option<String>,
        timezone: Option<String>,
        product_key: Option<String>,
    },
    Reboot {
        #[serde(default)]
        delay_seconds: u32,
    },
}

fn default_esp_size() -> u32 {
    260
}

fn default_msr_size() -> u32 {
    16
}

impl TaskSequence {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.name.trim().is_empty() {
            errors.push("Task sequence name must not be empty".into());
        }

        if self.steps.is_empty() {
            errors.push("Task sequence must have at least one step".into());
        }

        let mut has_partition = false;
        let mut has_apply = false;

        for (i, step) in self.steps.iter().enumerate() {
            if step.name.trim().is_empty() {
                errors.push(format!("Step {} name must not be empty", i + 1));
            }

            match step.step_type {
                StepType::Partition => has_partition = true,
                StepType::ApplyImage => {
                    if !has_partition {
                        errors.push(format!(
                            "Step {}: Apply Image must come after Partition",
                            i + 1
                        ));
                    }
                    has_apply = true;
                }
                StepType::InjectDrivers | StepType::GenerateUnattend => {
                    if !has_apply {
                        errors.push(format!(
                            "Step {}: {} must come after Apply Image",
                            i + 1,
                            step.step_type
                        ));
                    }
                }
                _ => {}
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
