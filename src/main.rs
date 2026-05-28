#![windows_subsystem = "windows"]

fn main() {
    #[cfg(target_os = "android")]
    panic!("Desktop main() called on Android");

    #[cfg(not(target_os = "android"))]
    {
        #[cfg(target_os = "macos")]
        {
            let store = apple_native_keyring_store::keychain::Store::new()
                .expect("Failed to initialize macOS keychain store");
            keyring_core::set_default_store(store);
        }
        #[cfg(target_os = "windows")]
        {
            let store = windows_native_keyring_store::Store::new()
                .expect("Failed to initialize Windows credential store");
            keyring_core::set_default_store(store);
        }
        #[cfg(target_os = "linux")]
        {
            let store = dbus_secret_service_keyring_store::Store::new()
                .expect("Failed to initialize Linux keyutils store");
            keyring_core::set_default_store(store);
        }
        let mut path = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        path.push("Talgonite");
        let _ = std::fs::create_dir_all(&path);
        talgonite_lib::main_with_storage(path);
    }
}
