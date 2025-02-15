use systemctl::SystemCtl;

const AGENT_UNIT_NAME: &str = "managers_agent";
const AGENT_UNIT_PATH: &str = "/etc/systemd/system";

const UNIT_TEMPLATE: &str = r#"
        [Unit]
        Description=sub-system Agent for manage-rs service
        Wants=network-online.target
        After=network.target network-online.target
        
        [Service]
        Type=simple
        ExecStart=/usr/bin/managers_agent
        
        
        [Install]
        WantedBy=multi-user.target
        
    "#;

pub fn init_systemd() -> eyre::Result<()> {
    let systemd = SystemCtl::default();
    // if the unit already exists, restart it so the new binary will take effect
    if systemd.exists(AGENT_UNIT_NAME)? {
        systemd.daemon_reload()?;
        systemd.restart(AGENT_UNIT_NAME)?;
        return Ok(());
    }

    std::fs::write(
        &format!("{AGENT_UNIT_PATH}/{AGENT_UNIT_NAME}.service"),
        UNIT_TEMPLATE.as_bytes(),
    )?;
    systemd.daemon_reload()?;

    systemd.enable(AGENT_UNIT_NAME)?;
    // we dont start the agent on out own, the server should do this instead.
    
    //systemd.restart(AGENT_UNIT_NAME)?;

    Ok(())
}
