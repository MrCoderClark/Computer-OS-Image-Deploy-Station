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

#[cfg(test)]
mod tests {
    use super::*;

    fn step(st: StepType, name: &str, order: u32) -> TaskStep {
        let config = match st {
            StepType::Partition => StepConfig::Partition { esp_size_mb: 260, msr_size_mb: 16 },
            StepType::ApplyImage => StepConfig::ApplyImage { image_index: 1 },
            StepType::InjectDrivers => StepConfig::InjectDrivers { driver_pack_id: None, driver_path: None },
            StepType::RunScript => StepConfig::RunScript { script_path: "test.ps1".into(), arguments: vec![], run_elevated: false },
            StepType::DomainJoin => StepConfig::DomainJoin { domain_name: "corp.local".into(), ou_path: None, account: "svc".into() },
            StepType::GenerateUnattend => StepConfig::GenerateUnattend { computer_name: None, locale: None, timezone: None, product_key: None },
            StepType::Reboot => StepConfig::Reboot { delay_seconds: 0 },
        };
        TaskStep {
            id: Uuid::new_v4(),
            step_type: st,
            name: name.into(),
            enabled: true,
            order,
            config,
        }
    }

    fn seq(steps: Vec<TaskStep>) -> TaskSequence {
        let now = chrono::Utc::now();
        TaskSequence {
            id: Uuid::new_v4(),
            name: "Test Sequence".into(),
            description: String::new(),
            steps,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn valid_partition_apply_reboot() {
        let s = seq(vec![
            step(StepType::Partition, "Partition", 0),
            step(StepType::ApplyImage, "Apply", 1),
            step(StepType::Reboot, "Reboot", 2),
        ]);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn valid_full_sequence() {
        let s = seq(vec![
            step(StepType::Partition, "Partition", 0),
            step(StepType::ApplyImage, "Apply", 1),
            step(StepType::InjectDrivers, "Drivers", 2),
            step(StepType::GenerateUnattend, "Unattend", 3),
            step(StepType::DomainJoin, "Join", 4),
            step(StepType::RunScript, "Script", 5),
            step(StepType::Reboot, "Reboot", 6),
        ]);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn empty_name_rejected() {
        let mut s = seq(vec![step(StepType::Partition, "P", 0)]);
        s.name = String::new();
        let errors = s.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("name")));
    }

    #[test]
    fn empty_steps_rejected() {
        let s = seq(vec![]);
        let errors = s.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("at least one step")));
    }

    #[test]
    fn apply_before_partition_rejected() {
        let s = seq(vec![
            step(StepType::ApplyImage, "Apply", 0),
            step(StepType::Partition, "Partition", 1),
        ]);
        let errors = s.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("Apply Image must come after Partition")));
    }

    #[test]
    fn inject_drivers_before_apply_rejected() {
        let s = seq(vec![
            step(StepType::Partition, "Partition", 0),
            step(StepType::InjectDrivers, "Drivers", 1),
        ]);
        let errors = s.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("Inject Drivers must come after Apply Image")));
    }

    #[test]
    fn unattend_before_apply_rejected() {
        let s = seq(vec![
            step(StepType::Partition, "Partition", 0),
            step(StepType::GenerateUnattend, "Unattend", 1),
        ]);
        let errors = s.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("Generate Unattend.xml must come after Apply Image")));
    }

    #[test]
    fn step_with_empty_name_rejected() {
        let s = seq(vec![
            step(StepType::Partition, "", 0),
        ]);
        let errors = s.validate().unwrap_err();
        assert!(errors.iter().any(|e| e.contains("Step 1 name")));
    }

    #[test]
    fn reboot_only_is_valid() {
        let s = seq(vec![step(StepType::Reboot, "Reboot", 0)]);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn run_script_anywhere_is_valid() {
        let s = seq(vec![step(StepType::RunScript, "Pre-script", 0)]);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn step_type_display() {
        assert_eq!(StepType::Partition.to_string(), "Partition Disk");
        assert_eq!(StepType::ApplyImage.to_string(), "Apply Image");
        assert_eq!(StepType::Reboot.to_string(), "Reboot");
    }

    #[test]
    fn sequence_roundtrip_json() {
        let s = seq(vec![
            step(StepType::Partition, "Partition", 0),
            step(StepType::ApplyImage, "Apply", 1),
        ]);
        let json = serde_json::to_string(&s).unwrap();
        let parsed: TaskSequence = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "Test Sequence");
        assert_eq!(parsed.steps.len(), 2);
        assert_eq!(parsed.steps[0].step_type, StepType::Partition);
        assert_eq!(parsed.steps[1].step_type, StepType::ApplyImage);
    }

    #[test]
    fn step_config_partition_defaults() {
        let json = r#"{"type":"partition"}"#;
        let cfg: StepConfig = serde_json::from_str(json).unwrap();
        match cfg {
            StepConfig::Partition { esp_size_mb, msr_size_mb } => {
                assert_eq!(esp_size_mb, 260);
                assert_eq!(msr_size_mb, 16);
            }
            _ => panic!("wrong variant"),
        }
    }
}
