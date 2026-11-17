use anyhow::{Context, Result};
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::Cursor;
use std::path::Path;

const MS_SHELL_SETUP: &str = "Microsoft-Windows-Shell-Setup";
const MS_INTL: &str = "Microsoft-Windows-International-Core";
const MS_UNATTEND_JOIN: &str = "Microsoft-Windows-UnattendedJoin";

pub struct UnattendConfig {
    pub computer_name: Option<String>,
    pub locale: String,
    pub timezone: String,
    pub product_key: Option<String>,
    pub admin_password: Option<String>,
    pub domain_join: Option<DomainJoinConfig>,
}

pub struct DomainJoinConfig {
    pub domain_name: String,
    pub ou_path: Option<String>,
    pub account: String,
    pub password: String,
}

impl Default for UnattendConfig {
    fn default() -> Self {
        Self {
            computer_name: None,
            locale: "en-US".into(),
            timezone: "Eastern Standard Time".into(),
            product_key: None,
            admin_password: None,
            domain_join: None,
        }
    }
}

pub fn generate_unattend_xml(config: &UnattendConfig) -> Result<String> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
        .context("write xml decl")?;

    let mut unattend = BytesStart::new("unattend");
    unattend.push_attribute(("xmlns", "urn:schemas-microsoft-com:unattend"));
    writer
        .write_event(Event::Start(unattend))
        .context("write unattend start")?;

    write_specialize_pass(&mut writer, config)?;
    write_oobesystem_pass(&mut writer, config)?;

    writer
        .write_event(Event::End(BytesEnd::new("unattend")))
        .context("write unattend end")?;

    let buf = writer.into_inner().into_inner();
    String::from_utf8(buf).context("xml not valid utf-8")
}

pub fn write_unattend_xml(config: &UnattendConfig, target_path: &Path) -> Result<()> {
    let xml = generate_unattend_xml(config)?;
    std::fs::write(target_path, &xml)
        .with_context(|| format!("Failed to write Unattend.xml to {}", target_path.display()))?;
    tracing::info!("Unattend.xml written to {}", target_path.display());
    Ok(())
}

fn write_specialize_pass(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    config: &UnattendConfig,
) -> Result<()> {
    let mut settings = BytesStart::new("settings");
    settings.push_attribute(("pass", "specialize"));
    writer.write_event(Event::Start(settings))?;

    // Shell Setup — computer name + product key
    if config.computer_name.is_some() || config.product_key.is_some() {
        write_component_start(writer, MS_SHELL_SETUP)?;

        if let Some(ref name) = config.computer_name {
            write_element(writer, "ComputerName", name)?;
        }
        if let Some(ref key) = config.product_key {
            write_element(writer, "ProductKey", key)?;
        }

        writer.write_event(Event::End(BytesEnd::new("component")))?;
    }

    // International — locale
    write_component_start(writer, MS_INTL)?;
    write_element(writer, "InputLocale", &config.locale)?;
    write_element(writer, "SystemLocale", &config.locale)?;
    write_element(writer, "UILanguage", &config.locale)?;
    write_element(writer, "UserLocale", &config.locale)?;
    writer.write_event(Event::End(BytesEnd::new("component")))?;

    // Domain join
    if let Some(ref dj) = config.domain_join {
        write_component_start(writer, MS_UNATTEND_JOIN)?;

        let id_start = BytesStart::new("Identification");
        writer.write_event(Event::Start(id_start))?;

        write_element(writer, "JoinDomain", &dj.domain_name)?;
        if let Some(ref ou) = dj.ou_path {
            write_element(writer, "MachineObjectOU", ou)?;
        }

        let creds_start = BytesStart::new("Credentials");
        writer.write_event(Event::Start(creds_start))?;
        write_element(writer, "Domain", &dj.domain_name)?;
        write_element(writer, "Username", &dj.account)?;
        write_element(writer, "Password", &dj.password)?;
        writer.write_event(Event::End(BytesEnd::new("Credentials")))?;

        writer.write_event(Event::End(BytesEnd::new("Identification")))?;
        writer.write_event(Event::End(BytesEnd::new("component")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("settings")))?;
    Ok(())
}

fn write_oobesystem_pass(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    config: &UnattendConfig,
) -> Result<()> {
    let mut settings = BytesStart::new("settings");
    settings.push_attribute(("pass", "oobeSystem"));
    writer.write_event(Event::Start(settings))?;

    write_component_start(writer, MS_SHELL_SETUP)?;

    // OOBE settings — skip user screens
    let oobe_start = BytesStart::new("OOBE");
    writer.write_event(Event::Start(oobe_start))?;
    write_element(writer, "HideEULAPage", "true")?;
    write_element(writer, "HideLocalAccountScreen", "true")?;
    write_element(writer, "HideOnlineAccountScreens", "true")?;
    write_element(writer, "HideWirelessSetupInOOBE", "true")?;
    write_element(writer, "ProtectYourPC", "3")?;
    writer.write_event(Event::End(BytesEnd::new("OOBE")))?;

    write_element(writer, "TimeZone", &config.timezone)?;

    // Admin password
    if let Some(ref pwd) = config.admin_password {
        let pwd_start = BytesStart::new("UserAccounts");
        writer.write_event(Event::Start(pwd_start))?;
        let admin_start = BytesStart::new("AdministratorPassword");
        writer.write_event(Event::Start(admin_start))?;
        write_element(writer, "Value", pwd)?;
        write_element(writer, "PlainText", "true")?;
        writer.write_event(Event::End(BytesEnd::new("AdministratorPassword")))?;
        writer.write_event(Event::End(BytesEnd::new("UserAccounts")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("component")))?;
    writer.write_event(Event::End(BytesEnd::new("settings")))?;
    Ok(())
}

fn write_component_start(writer: &mut Writer<Cursor<Vec<u8>>>, name: &str) -> Result<()> {
    let mut comp = BytesStart::new("component");
    comp.push_attribute(("name", name));
    comp.push_attribute(("processorArchitecture", "amd64"));
    comp.push_attribute(("publicKeyToken", "31bf3856ad364e35"));
    comp.push_attribute(("language", "neutral"));
    comp.push_attribute(("versionScope", "nonSxS"));
    writer.write_event(Event::Start(comp))?;
    Ok(())
}

fn write_element(writer: &mut Writer<Cursor<Vec<u8>>>, tag: &str, value: &str) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new(tag)))?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new(tag)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_basic_unattend() {
        let config = UnattendConfig {
            computer_name: Some("WORKSTATION-01".into()),
            locale: "en-US".into(),
            timezone: "Eastern Standard Time".into(),
            product_key: None,
            admin_password: None,
            domain_join: None,
        };

        let xml = generate_unattend_xml(&config).unwrap();
        assert!(xml.contains("WORKSTATION-01"));
        assert!(xml.contains("specialize"));
        assert!(xml.contains("oobeSystem"));
        assert!(xml.contains("en-US"));
    }

    #[test]
    fn generates_domain_join_unattend() {
        let config = UnattendConfig {
            computer_name: Some("PC-001".into()),
            domain_join: Some(DomainJoinConfig {
                domain_name: "corp.example.com".into(),
                ou_path: Some("OU=Workstations,DC=corp,DC=example,DC=com".into()),
                account: "svc-join".into(),
                password: "secret".into(),
            }),
            ..Default::default()
        };

        let xml = generate_unattend_xml(&config).unwrap();
        assert!(xml.contains("corp.example.com"));
        assert!(xml.contains("JoinDomain"));
        assert!(xml.contains("MachineObjectOU"));
    }
}
