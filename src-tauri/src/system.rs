use std::{path::Path, process::Command};

const FIREWALL_RULE: &str = "LanClip Clipboard Sync";

pub fn firewall_ready(executable: &Path) -> bool {
    #[cfg(windows)]
    {
        let path = ps_quote(&executable.to_string_lossy());
        let script = format!(
            "$p='{path}';$r=Get-NetFirewallRule -DisplayName '{FIREWALL_RULE}' \
             -ErrorAction SilentlyContinue|Where-Object{{$_.Enabled-eq'True'-and$_.Direction-eq'Inbound'-and$_.Action-eq'Allow'}};\
             $r=@($r|Where-Object{{(Get-NetFirewallApplicationFilter -AssociatedNetFirewallRule $_).Program-eq$p}});\
             $ok=@($r|ForEach-Object{{Get-NetFirewallPortFilter -AssociatedNetFirewallRule $_}}|\
             Where-Object{{($_.Protocol-eq'TCP'-and$_.LocalPort-eq'44778')-or($_.Protocol-eq'UDP'-and$_.LocalPort-eq'44777')}}).Count-ge2;\
             if($ok){{'READY'}}"
        );
        return powershell_output(&script).is_some_and(|output| output.contains("READY"));
    }
    #[cfg(not(windows))]
    {
        let _ = executable;
        true
    }
}

pub fn request_firewall_access(executable: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let path = ps_quote(&executable.to_string_lossy());
        let elevated_script = format!(
            "Get-NetFirewallRule -DisplayName '{FIREWALL_RULE}' -ErrorAction SilentlyContinue|Remove-NetFirewallRule;\
             New-NetFirewallRule -DisplayName '{FIREWALL_RULE}' -Direction Inbound -Action Allow -Profile Any \
             -Program '{path}' -Protocol TCP -LocalPort 44778|Out-Null;\
             New-NetFirewallRule -DisplayName '{FIREWALL_RULE}' -Direction Inbound -Action Allow -Profile Any \
             -Program '{path}' -Protocol UDP -LocalPort 44777|Out-Null"
        );
        let argument = format!(
            "-NoProfile -ExecutionPolicy Bypass -Command \"{}\"",
            elevated_script.replace('"', "\\\"")
        );
        let status = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-WindowStyle",
                "Hidden",
                "-Command",
                "Start-Process",
                "powershell.exe",
                "-Verb",
                "RunAs",
                "-Wait",
                "-ArgumentList",
                &argument,
            ])
            .status()
            .map_err(|e| format!("无法打开防火墙授权窗口：{e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("防火墙授权被取消或执行失败".into())
        }
    }
    #[cfg(not(windows))]
    {
        let _ = executable;
        Ok(())
    }
}

pub fn installed_mode(executable: &Path) -> bool {
    let path = executable.to_string_lossy().to_ascii_lowercase();
    !path.contains("\\target\\") && !path.contains("/target/")
}

#[cfg(windows)]
fn powershell_output(script: &str) -> Option<String> {
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", script])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

fn ps_quote(value: &str) -> String {
    value.replace('\'', "''")
}
