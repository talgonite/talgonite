#![windows_subsystem = "windows"]

fn main() {
    #[cfg(target_os = "android")]
    panic!("Desktop main() called on Android");

    #[cfg(not(target_os = "android"))]
    {
        let mut path = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        path.push("Talgonite");
        let _ = std::fs::create_dir_all(&path);
        talgonite_lib::main_with_storage(path);
    }
}
