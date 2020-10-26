use crate::http_client::HttpClient;
use crate::mime::mime_type_by_extension;
use crate::request::Request;
use deflate::{deflate_bytes, deflate_bytes_gzip};
use std::collections::btree_map::BTreeMap;
use std::fs::{read_dir, File, Metadata};
use std::io;
use std::io::ErrorKind;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::thread::{sleep, spawn};
use std::time::{Duration, SystemTime};

/// Dynamic cache in the RAM of files on disk.
/// It stores the files of the specified directory loaded in the RAM, monitors difference of
/// directory on disk and cache with some periodicity and updates the contents in the RAM.
/// Files are stored in the RAM with their original data and/or as compressed.
/// Directory monitoring is done in its own background thread (by default) or manually.
/// Also it's manages browser-side cache.
/// Can be used in multi-threaded environment after clone.
/// Can't long time blocking operations when update cache.
/// For manually settings some parameters see 'Builder'.
#[derive(Clone)]
pub struct StaticFiles {
    /// Path to directory that will be cached in the RAM.
    dir_path: String,
    /// Cached files data in the RAM and related information.
    cached_files: Arc<RwLock<BTreeMap<String, StaticFile>>>,

    /// Need cache data as deflate compressed.
    deflate_encoding: bool,
    /// Need cache data as gzip compressed.
    gzip_encoding: bool,
    /// Need sending of "Last-Modified" header for browser cache and check changes.
    use_last_modified: bool,
    /// Need sending of "ETag" header and changes checking for browser cache.
    use_etag: bool,

    /// To try send small data in one write operation if data len less then this parameter.
    united_response_limit: usize,
}

/// Cached file data and related information in the the RAM.
#[derive(Clone)]
pub struct StaticFile {
    /// Raw file data.
    raw_data: Arc<Vec<u8>>,
    /// File data as deflate compressed.
    deflate_data: Option<Arc<Vec<u8>>>,
    /// File data as gzip compressed.
    gzip_data: Option<Arc<Vec<u8>>>,

    /// Prepared content type string for http response header "Content-Type".
    content_type: String,

    /// Last modified time of file on disk taken from file metadata.
    last_modified: SystemTime,
    /// Prepared string for value of http response header "Last-Modified".
    last_modified_rfc7231: String,
    /// Prepared string for value of "ETag" header. md5 of all raw file data.
    etag: String,
}

impl StaticFiles {
    /// Creates new dynamic cache in RAM of files on disk in `path` directory.
    pub fn new(path: &str) -> Self {
        StaticFiles::from_builder(path, &Builder::default())
    }

    /// Creates new `Self` with parameters specified in builder.
    pub fn from_builder(path: &str, builder: &Builder) -> Self {
        let cached_files = Arc::new(RwLock::new(BTreeMap::new()));

        let static_files = StaticFiles {
            dir_path: path.to_string(),
            cached_files,
            deflate_encoding: builder.deflate_encoding,
            gzip_encoding: builder.gzip_encoding,
            use_last_modified: builder.use_last_modified,
            use_etag: builder.use_etag,
            united_response_limit: builder.united_response_limit,
        };

        let result = static_files.clone();

        if !builder.deferred_load {
            static_files.update();
        }

        if let Some(interval) = builder.updating_interval {
            spawn(move || {
                loop {
                    sleep(interval);
                    static_files.update();
                }
            });
        }

        result
    }

    /// Send response with file content to the client.
    pub fn response(&self, path: &str, request: &Request, client: &HttpClient) -> io::Result<()> {
        let mut result = Ok(());

        self.get(path, |static_file| {
            // this code is under read blocking of RwLock of all files
            match static_file {
                Some(static_file) => {
                    let mut apply_browser_cache = false;
                    if !static_file.etag.is_empty() {
                        if let Some(if_none_match) = request.header_value("If-None-Match") {
                            if static_file.etag == if_none_match {
                                apply_browser_cache = true;
                            }
                        }
                    } else if !static_file.last_modified_rfc7231.is_empty() {
                        if let Some(if_modified_since) = request.header_value("If-Modified-Since") {
                            if static_file.last_modified_rfc7231 == if_modified_since {
                                apply_browser_cache = true;
                            }
                        }
                    }

                    if apply_browser_cache {
                        // browser cache will be applied
                        let response = Vec::from(format!(
                            "{} 304 Not Modified\r\n\
                             Date: {}\r\n\
                             {}\
                             {}\
                             {}\
                             \r\n",
                            request.version.to_string_for_response(),
                            client.http_date_string(),
                            crate::response::connection_str_by_request(request),
                            if static_file.last_modified_rfc7231.is_empty() { "".to_string() } else { format!("Last-Modified: {}\r\n", static_file.last_modified_rfc7231) },
                            if static_file.etag.is_empty() { "".to_string() } else { format!("ETag: {}\r\n", static_file.etag) }
                        ));

                        client.response_raw(&response);
                        return;
                    }

                    let mut content = &static_file.raw_data;
                    let mut content_header = "";
                    if let Some(encoding) = request.header_value("Accept-Encoding") {
                        if let Some(deflate_data) = &static_file.deflate_data {
                            if encoding.contains("deflate") {
                                content = &deflate_data;
                                content_header = "Content-Encoding: deflate\r\n";
                            }
                        } else if let Some(gzip_data) = &static_file.gzip_data {
                            if encoding.contains("gzip") {
                                content = &gzip_data;
                                content_header = "Content-Encoding: gzip\r\n";
                            }
                        }
                    }

                    let mut response = Vec::from(format!(
                        "{} 200 OK\r\n\
                         Date: {}\r\n\
                         {}\
                         {}\
                         {}\
                         {}\
                         Content-Length: {}\r\n\
                         Content-Type: {}\r\n\
                         \r\n",
                        request.version.to_string_for_response(),
                        client.http_date_string(),
                        crate::response::connection_str_by_request(request),
                        content_header,
                        if static_file.last_modified_rfc7231.is_empty() { "".to_string() } else { format!("Last-Modified: {}\r\n", static_file.last_modified_rfc7231) },
                        if static_file.etag.is_empty() { "".to_string() } else { format!("ETag: {}\r\n", static_file.etag) },
                        content.len(),
                        static_file.content_type
                    ));

                    if content.len() < self.united_response_limit {
                        response.extend(&content[..]);
                        client.response_raw(&response);
                    } else {
                        client.response_raw(&response);
                        client.response_raw_arc(&content);
                    }
                }
                None => {
                    result = Err(io::Error::new(ErrorKind::NotFound, "No such static file"))
                }
            }
        });

        result
    }

    /// Return current cached files paths.
    pub fn files(&self) -> Vec<String> {
        let mut result = vec![];
        match self.cached_files.read() {
            Ok(cached_files) => {
                //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
                for cached_file in cached_files.keys() {
                    result.push(cached_file.clone());
                }
                //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
            }
            Err(err) => {
                dbg!("unreachable code");
                dbg!(err);
            }
        }

        result
    }

    /// Updating the RAM cache in accordance with directory on the disk. It's execute in call thread.
    pub fn update(&self) {
        self.remove_nonexistent();
        self.update_dir("");
    }

    /// Recursive update the RAM cache in accordance with directory on the disk.
    fn update_dir(&self, subdir_path: &str) {
        let mut cur_dir_path = self.dir_path.clone();
        if !subdir_path.is_empty() {
            cur_dir_path.push('/');
            cur_dir_path += &subdir_path;
        }

        match read_dir(&cur_dir_path) {
            Ok(paths) => {
                for path in paths {
                    if let Ok(path) = path {
                        if let Ok(metadata) = path.metadata() {
                            if let Some(name) = path.file_name().to_str() {
                                let mut path_with_subdirs = subdir_path.to_owned();
                                if !path_with_subdirs.is_empty() {
                                    path_with_subdirs.push('/');
                                }
                                path_with_subdirs += name;

                                if metadata.is_file() {
                                    self.check_file_and_cache_if_need(&path_with_subdirs, &metadata);
                                } else if metadata.is_dir() {
                                    // recurse subdirectory
                                    self.update_dir(&path_with_subdirs);
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {
                self.clear();
            }
        }
    }

    /// Get static file data from cache by path. Callback under read blocking of RwLock of files container.
    fn get(&self, file_path: &str, mut result_callback: impl FnMut(Option<&StaticFile>)) {
        let file_name = if file_path.starts_with('/') { &file_path[1..] } else { file_path };

        match self.cached_files.read() {
            Ok(cached_files) => {
                //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
                if let Some(static_file) = cached_files.get(file_name) {
                    result_callback(Some(static_file));
                    return;
                }
                //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
            }
            Err(err) => {
                dbg!("unreachable code");
                dbg!(err);
            }
        }

        result_callback(None);
    }

    /// Remove from cache nonexistent files in directory on disk.
    fn remove_nonexistent(&self) {
        let mut nonexistent = vec![];
        match self.cached_files.read() {
            Ok(cached_files) => {
                //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
                for file_name in cached_files.keys() {
                    if !Path::new(&(self.dir_path.clone() + "/" + file_name)).exists() {
                        nonexistent.push(file_name.clone());
                    }
                }
                //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
            }
            Err(err) => {
                dbg!("unreachable code");
                dbg!(err);
            }
        }

        if nonexistent.is_empty() {
            return;
        }

        match self.cached_files.write() {
            Ok(mut cached_files) => {
                //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
                for file_name in nonexistent {
                    cached_files.remove(&file_name);
                }
                //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
            }
            Err(err) => {
                dbg!("unreachable code");
                dbg!(err);
            }
        }
    }

    /// Checks of difference of file on the disk and in the RAM and update cache if need.
    fn check_file_and_cache_if_need(&self, file_path: &str, metadata: &Metadata) {
        if let Ok(modified) = metadata.modified() {
            let mut last_modified = None;

            match self.cached_files.read() {
                Ok(cached_files) => {
                    //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
                    if let Some(cached_file) = cached_files.get(file_path) {
                        last_modified = Some(cached_file.last_modified);
                    }
                    //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
                }
                Err(err) => {
                    dbg!("unreachable code");
                    dbg!(err);
                }
            }

            match last_modified {
                Some(last_modified) => {
                    if modified > last_modified {
                        // update cached data
                        self.cache(file_path, &modified);
                    }
                }
                None => {
                    // cache it if not cached yet
                    self.cache(file_path, &modified);
                }
            }
        }
    }

    /// Loading and preparing file data and write to the RAM cache.
    fn cache(&self, file_path: &str, modified: &SystemTime) {
        // cache it if not cached yet
        if let Ok(mut file) = File::open(self.dir_path.clone() + "/" + file_path) {
            let mut raw_data = vec![];
            if file.read_to_end(&mut raw_data).is_ok() {
                let file_name = file_path.to_string();

                let mut extension = String::new();
                if let Some(e) = Path::new(file_path).extension() {
                    if let Some(e) = e.to_str() {
                        extension = e.to_string();
                    }
                }

                let content_type = mime_type_by_extension(&extension).to_string();

                let deflate_data = if self.deflate_encoding { Some(Arc::new(deflate_bytes(&raw_data))) } else { None };

                let gzip_data = if self.gzip_encoding { Some(Arc::new(deflate_bytes_gzip(&raw_data))) } else { None };

                let last_modified_rfc7231 = if self.use_last_modified { chrono::DateTime::<chrono::Utc>::from(*modified).to_rfc2822().replace("+0000", "GMT") } else { "".to_string() };

                let etag = if self.use_etag { format!("{:x}", md5::compute(&raw_data)) } else { "".to_string() };

                let cached_file = StaticFile {
                    raw_data: Arc::new(raw_data),
                    deflate_data,
                    gzip_data,
                    content_type,
                    last_modified: *modified,
                    last_modified_rfc7231,
                    etag,
                };

                // short blocking
                match self.cached_files.write() {
                    Ok(mut cached_files) => {
                        //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
                        cached_files.insert(file_name, cached_file);
                        //=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=
                    }
                    Err(err) => {
                        dbg!("unreachable code");
                        dbg!(err);
                    }
                }
            }
        }
    }

    /// Clear cache. It's calling when updating cache and no directory on the disk.
    fn clear(&self) {
        match self.cached_files.write() {
            Ok(mut cached_files) => {
                //=-=-=-=-=-=-=-=-=-=
                cached_files.clear();
                //=-=-=-=-=-=-=-=-=-=
            }
            Err(err) => {
                dbg!("unreachable code");
                dbg!(err);
            }
        }
    }
}

/// Builder of `StaticFiles`.
pub struct Builder {
    /// Interval of scanning directory and cache updating in background thread.
    /// If interval is None, then no background thread is create.
    /// If it's None and `Self::deferred_load` is true then content will loaded only
    /// after manually call `StaticFile::update()` function.
    pub updating_interval: Option<Duration>,
    /// Will store and response file data as deflate compressed.
    pub deflate_encoding: bool,
    /// Will store and response file data as gzip compressed.
    pub gzip_encoding: bool,
    /// Enable/disable using browser cache with "Last-Modified" header.
    pub use_last_modified: bool,
    /// Enable/disable using browser cache with "ETag" header.
    pub use_etag: bool,
    /// If false then content will loading to the RAM and prepared in current thread when creating.
    /// If true then content will loading in background thread after `updating_interval` or with
    /// manually call `StaticFile::update()` function.
    pub deferred_load: bool,
    /// To try send small data in one write operation if data len less then this parameter.
    pub united_response_limit: usize,
}

impl Default for Builder {
    fn default() -> Builder {
        Builder {
            updating_interval: Some(Duration::from_secs(1)),
            deflate_encoding: true,
            gzip_encoding: true,
            use_last_modified: true,
            use_etag: true,
            united_response_limit: 200000,
            deferred_load: false,
        }
    }
}

impl Builder {
    /// Creates builder of `StaticFiles` with default settings.
    pub fn new() -> Self {
        Builder::default()
    }

    /// Creates `StaticFiles` from builder. `path` - path to directory on disk that will be cached.
    pub fn build(&self, path: &str) -> StaticFiles {
        StaticFiles::from_builder(path, &self)
    }

    /// Interval of scanning directory and cache updating in background thread.
    /// If interval is None, then no background thread is create.
    /// If it's None and `Self::deferred_load` is true then content will loaded only
    /// after manually call `StaticFile::update()` function.
    pub fn updating_interval(mut self, interval: Option<Duration>) -> Self {
        self.updating_interval = interval;
        self
    }

    /// Will store and response data as deflate compressed.
    pub fn deflate_encoding(mut self, enabled: bool) -> Self {
        self.deflate_encoding = enabled;
        self
    }

    /// Will store and response data as gzip compressed.
    pub fn gzip_encoding(mut self, enabled: bool) -> Self {
        self.gzip_encoding = enabled;
        self
    }

    /// Enable/disable using browser cache with "Last-Modified" header.
    pub fn use_last_modified(mut self, enabled: bool) -> Self {
        self.use_last_modified = enabled;
        self
    }

    /// Enable/disable using browser cache with "ETag" header.
    pub fn use_etag(mut self, enabled: bool) -> Self {
        self.use_etag = enabled;
        self
    }

    /// If false then content will loading to the RAM and prepared in current thread when creating.
    /// If true then content will loading in background thread after `updating_interval` or with
    /// manually call update function.
    pub fn deferred_load(mut self, deferred: bool) -> Self {
        self.deferred_load = deferred;
        self
    }

    /// To try send small data in one write operation if data len less then this parameter.
    pub fn united_response_limit(mut self, size: usize) -> Self {
        self.united_response_limit = size;
        self
    }
}
