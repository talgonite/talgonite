#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct ArxArchive {
    archive: std::sync::Arc<libarx::Arx>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
pub struct WebArchive {
    base_url: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl ArxArchive {
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        use std::sync::Arc;

        let archive = libarx::Arx::new(path)?;
        Ok(Self {
            archive: Arc::new(archive),
        })
    }

    pub fn get_file(&self, path: &str) -> Result<Vec<u8>, ArxError> {
        use jubako as jbk;
        use libarx::{self as arx, FullBuilder};
        use std::io::Read;

        if let Ok(arx::Entry::File(content_address)) =
            self.archive.get_entry::<FullBuilder>(arx::Path::new(path))
        {
            if let jbk::Result::Ok(Some(jbk::reader::MayMissPack::FOUND(Some(bytes)))) =
                self.archive.get_bytes(content_address.content())
            {
                let mut buf = vec![];
                bytes.stream().read_to_end(&mut buf)?;
                return Ok(buf);
            }
        }

        Err(ArxError::FileNotFound(path.to_string()))
    }

    pub fn get_file_or_panic(&self, path: &str) -> Vec<u8> {
        match self.get_file(path) {
            Ok(data) => data,
            Err(e) => panic!("Failed to get file '{}': {}", path, e),
        }
    }

    // Async versions for consistent API with WebArchive
    pub async fn get_file_async(&self, path: &str) -> Result<Vec<u8>, ArxError> {
        self.get_file(path)
    }

    pub async fn get_file_or_panic_async(&self, path: &str) -> Vec<u8> {
        self.get_file_or_panic(path)
    }
}

#[cfg(target_arch = "wasm32")]
impl WebArchive {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }

    pub async fn get_file(&self, path: &str) -> Result<Vec<u8>, ArxError> {
        let url = format!("{}/{}", self.base_url, path);
        self.fetch_file(&url).await
    }

    // New: fetch multiple files concurrently (panic on error for parity with *_or_panic)
    pub async fn get_files_parallel(&self, paths: &[String]) -> Vec<Vec<u8>> {
        use futures::future::join_all;
        let urls: Vec<String> = paths
            .iter()
            .map(|p| format!("{}/{}", self.base_url, p))
            .collect();
        let futs = urls.iter().map(|u| self.fetch_file(u));
        let results = join_all(futs).await;
        results
            .into_iter()
            .map(|r| r.expect("Failed to fetch one of the parallel files"))
            .collect()
    }

    #[cfg(target_arch = "wasm32")]
    async fn fetch_file(&self, url: &str) -> Result<Vec<u8>, ArxError> {
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::JsFuture;
        use web_sys::{Request, RequestInit, RequestMode, Response};

        let opts = RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(RequestMode::Cors);

        let request = Request::new_with_str_and_init(url, &opts)
            .map_err(|_| ArxError::ArchiveError(format!("HTTP request failed: builder error")))?;

        let resp = JsFuture::from(web_sys::window().unwrap().fetch_with_request(&request))
            .await
            .map_err(|_| ArxError::ArchiveError(format!("HTTP request failed: fetch error")))?;

        let resp: Response = resp
            .dyn_into()
            .map_err(|_| ArxError::ArchiveError(format!("HTTP request failed: cast error")))?;

        if !resp.ok() {
            return Err(ArxError::FileNotFound(url.to_string()));
        }

        let buffer = JsFuture::from(resp.array_buffer().unwrap())
            .await
            .map_err(|_| {
                ArxError::ArchiveError(format!("Failed to read response: array buffer error"))
            })?;

        let data = web_sys::js_sys::Uint8Array::new(&buffer).to_vec();
        Ok(data)
    }

    pub async fn get_file_or_panic(&self, path: &str) -> Vec<u8> {
        match self.get_file(path).await {
            Ok(data) => data,
            Err(e) => panic!("Failed to get file '{}': {}", path, e),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ArxError {
    FileNotFound(String),
    IoError(String),
    ArchiveError(String),
}

impl std::fmt::Display for ArxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArxError::FileNotFound(path) => write!(f, "File not found: {}", path),
            ArxError::IoError(err) => write!(f, "IO error: {}", err),
            ArxError::ArchiveError(msg) => write!(f, "Archive error: {}", msg),
        }
    }
}

impl std::error::Error for ArxError {}

impl From<std::io::Error> for ArxError {
    fn from(err: std::io::Error) -> Self {
        ArxError::IoError(err.to_string())
    }
}

// Simple wrapper that can be used by both desktop and web
pub struct GameFiles {
    #[cfg(not(target_arch = "wasm32"))]
    archive: ArxArchive,
    #[cfg(target_arch = "wasm32")]
    archive: WebArchive,
}

impl GameFiles {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(archive_path: &str) -> Self {
        let archive = ArxArchive::new(archive_path).expect("Failed to open game archive");
        Self { archive }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_archive(archive: ArxArchive) -> Self {
        Self { archive }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(base_url: String) -> Self {
        let archive = WebArchive::new(base_url);
        Self { archive }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn archive(&self) -> &ArxArchive {
        &self.archive
    }

    #[cfg(target_arch = "wasm32")]
    pub fn archive(&self) -> &WebArchive {
        &self.archive
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_file_or_panic(&self, path: &str) -> Vec<u8> {
        self.archive.get_file_or_panic(path)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn get_file(&self, path: &str) -> Option<Vec<u8>> {
        self.archive.get_file(path).ok()
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn get_file(&self, path: &str) -> Vec<u8> {
        self.archive.get_file_or_panic(path).await
    }

    // New: parallel fetch proxy for wasm
    #[cfg(target_arch = "wasm32")]
    pub async fn get_files_parallel(&self, paths: &[String]) -> Vec<Vec<u8>> {
        self.archive.get_files_parallel(paths).await
    }
}
