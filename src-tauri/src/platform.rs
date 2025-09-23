#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

#[cfg(target_os = "macos")]
pub fn check_accessibility_permissions() {
    let trusted = unsafe { AXIsProcessTrusted() };
    if !trusted {
        println!("⚠️  Commander needs Accessibility permissions to simulate paste and global shortcuts.");
        println!("   Please grant permissions in:");
        println!("   System Settings → Privacy & Security → Accessibility");
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    } else {
        println!("✅ Accessibility permissions appear to be granted");
    }
}

#[cfg(target_os = "macos")]
pub fn is_accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

#[cfg(target_os = "macos")]
pub fn open_accessibility_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}


