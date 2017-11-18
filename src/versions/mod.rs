#![allow(dead_code)]
#![allow(unreachable_patterns)]

use std::io;
use std::fs;
use std::fmt;
use std::error;
use std::rc::Rc;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::result::Result;
use std::collections::HashMap;
use zip::read::ZipArchive;
use zip::result::ZipError;
use serde_json::{Value, self};
use serde::de::{Deserialize, Deserializer, Visitor, MapAccess, self};

use launcher;
use parsing;

#[cfg(target_pointer_width = "32")]
const OS_ARCH: &str = "32";
#[cfg(target_pointer_width = "64")]
const OS_ARCH: &str = "64";
#[cfg(target_os = "windows")]
const OS_PLATFORM: &str = "windows";
#[cfg(target_os = "macos")]
const OS_PLATFORM: &str = "macos";
#[cfg(target_os = "linux")]
const OS_PLATFORM: &str = "linux";

const CLASSPATH_SEPARATOR: &str = ":";

#[derive(Deserialize, Debug)]
pub struct MinecraftVersion {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
    #[serde(rename = "time")]
    publish_time: String,
    #[serde(rename = "releaseTime")]
    release_time: String,
    // TODO: 1.13+ arguments
    /*
    #[serde(default)]
    arguments: HashMap<String, String>,
    */
    #[serde(rename = "minecraftArguments")]
    minecraft_arguments: Option<String>,
    #[serde(rename = "mainClass", default)]
    main_class: Option<String>,
    #[serde(rename = "jar", default)]
    version_jar: Option<String>,
    #[serde(rename = "assets")]
    assets_id: Option<String>,
    #[serde(rename = "assetIndex")]
    asset_index: Option<AssetDownloadInfo>,
    #[serde(default)]
    assets: Option<String>,
    #[serde(default)]
    libraries: Vec<Library>,
    #[serde(default)]
    downloads: HashMap<String, DownloadInfo>,
    #[serde(rename = "inheritsFrom")]
    inherits_from: Option<String>,
}

#[derive(Debug)]
pub struct DownloadStrategy {
    with_classifier: HashMap<String, (String, DownloadInfo)>,
    default: Option<DownloadInfo>,
    rules: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct Library {
    name: String,
    is_native: bool,
    downloads: Rc<DownloadStrategy>,
    extract_ignored: Rc<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct NativeCollection {
    libraries: Vec<(PathBuf, Rc<Vec<String>>)>
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum DownloadInfo {
    PreHashed { size: i32, url: String, sha1: String },
    RawXzip { url: String },
    Raw { url: String },
}

#[derive(Deserialize, Clone, Debug)]
pub struct AssetDownloadInfo {
    size: Option<i32>,
    url: Option<String>,
    sha1: Option<String>,
    #[serde(rename = "id")]
    asset_index_id: String,
    #[serde(rename = "totalSize")]
    total_size: Option<i64>,
    #[serde(rename = "known", default)]
    size_and_hash_known: bool,
}

pub struct VersionManager(Box<Path>);

#[derive(Debug)]
pub enum Error {
    FileUnavailableError(Box<Path>),
    UnrecognizedPathString(OsString),
    IOError(Box<error::Error + Send + Sync>),
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::IOError(Box::new(e))
    }
}

impl From<OsString> for Error {
    fn from(e: OsString) -> Self {
        Error::UnrecognizedPathString(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IOError(Box::new(e))
    }
}

impl From<ZipError> for Error {
    fn from(e: ZipError) -> Self {
        Error::IOError(Box::new(io::Error::from(e)))
    }
}

impl NativeCollection {
    fn is_file_included(&self, extract_ignored: &Vec<String>, file_name: &str) -> bool {
        extract_ignored.iter().find(|rule| file_name.starts_with(rule.as_str())).is_none()
    }

    pub fn extract_to(&self, target_dir_path: &Path) -> Result<Vec<String>, Error> {
        let mut result = Vec::new();
        let target_path_buf = target_dir_path.to_path_buf();
        if !target_dir_path.is_dir() { fs::create_dir_all(target_dir_path)? }
        for &(ref path_buf, ref extract_ignored) in self.libraries.iter() {
            let zip_file = fs::File::open(path_buf)?;
            let mut zip = ZipArchive::new(zip_file)?;
            for i in 0..zip.len() {
                let mut source = zip.by_index(i)?;
                let file_name = source.name().to_owned();
                if self.is_file_included(&extract_ignored, file_name.as_str()) {
                    let target_path = target_path_buf.join(file_name.as_str());
                    let mut target = fs::File::create(target_path)?;
                    io::copy(&mut source, &mut target)?;
                    result.push(file_name);
                }
            }
        }
        Result::Ok(result)
    }
}

impl VersionManager {
    pub fn new(path: &Path) -> VersionManager {
        VersionManager(Box::from(path))
    }

    pub fn get_primary_jar_path(&self, id: &str) -> PathBuf {
        let sub_path = format!("{}.jar", id);
        let mut path_buf = self.0.join(id);
        path_buf.push(sub_path);
        path_buf
    }

    pub fn get_natives_path(&self, id: &str) -> PathBuf {
        let sub_path = format!("{}-natives-{}-{}/", id, OS_PLATFORM, OS_ARCH);
        let mut path_buf = self.0.join(id);
        path_buf.push(sub_path);
        path_buf
    }

    pub fn extract_natives(&self, id: &str, library_path: &Path) -> Result<Vec<String>, Error> {
        let info = self.version_of(id)?;
        let path_buf = self.get_natives_path(id);
        info.to_native_collection(self, library_path)?.extract_to(path_buf.as_path())
    }

    pub fn version_of(&self, id: &str) -> Result<MinecraftVersion, Error> {
        let path_buf = self.0.join(id);
        if !path_buf.is_dir() { fs::create_dir_all(path_buf.as_path())? }
        let path_buf_json = path_buf.join(format!("{}.json", id));
        if path_buf_json.exists() {
            Result::Ok(serde_json::from_reader(fs::File::open(path_buf_json)?)?)
        } else {
            Result::Err(Error::FileUnavailableError(path_buf_json.into_boxed_path()))
        }
    }
}

impl MinecraftVersion {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn version_type(&self) -> &str {
        &self.version_type
    }

    pub fn publish_time(&self) -> &str {
        &self.publish_time
    }

    pub fn release_time(&self) -> &str {
        &self.release_time
    }

    pub fn asset_index(&self, manager: &VersionManager) -> Option<AssetDownloadInfo> {
        self.asset_index.clone().or_else(|| self.assets_id.clone().map(AssetDownloadInfo::new)).or_else(|| {
            if let Some(ref inherits_from) = self.inherits_from {
                manager.version_of(&inherits_from).ok().and_then(|v| v.asset_index(manager))
            } else {
                None
            }
        })
    }

    pub fn main_class(&self, manager: &VersionManager) -> Option<String> {
        self.main_class.clone().or_else(|| {
            if let Some(ref inherits_from) = self.inherits_from {
                manager.version_of(&inherits_from).ok().and_then(|v| v.main_class(manager))
            } else {
                None
            }
        })
    }

    pub fn libraries(&self, manager: &VersionManager) -> Result<Vec<Library>, Error> {
        if let Some(ref inherits_from) = self.inherits_from {
            let mut result = manager.version_of(&inherits_from)?.libraries(manager)?;
            result.extend(self.libraries.clone().into_iter());
            Result::Ok(result)
        } else {
            Result::Ok(self.libraries.clone())
        }
    }

    pub fn version_jar(&self, manager: &VersionManager) -> Result<String, Error> {
        match self.version_jar {
            Some(ref jar) => Result::Ok(jar.to_owned()),
            None => if let Some(ref inherits_from) = self.inherits_from {
                manager.version_of(&inherits_from)?.version_jar(manager)
            } else {
                Result::Ok(self.id.to_owned())
            }
        }
    }

    pub fn collect_game_arguments(&self,
                                  manager: &VersionManager,
                                  parameters: &mut Vec<launcher::GameOption>,
                                  s: &parsing::ParameterStrategy) -> Result<(), Error> {
        let mut option_name = None;
        match self.minecraft_arguments {
            Some(ref args) => {
                for arg in parsing::parse(&args, s) {
                    if arg.is_empty() { return Result::Ok(()); }
                    match option_name {
                        None => if arg.starts_with("-") {
                            option_name = Some(arg);
                        } else {
                            (*parameters).push(launcher::GameOption::new_single(arg));
                        }
                        Some(name) => if arg.starts_with("-") {
                            (*parameters).push(launcher::GameOption::new_single(name));
                            option_name = Some(arg);
                        } else {
                            (*parameters).push(launcher::GameOption::new_pair(name, arg));
                            option_name = None;
                        }
                    }
                }
                if let Some(name) = option_name {
                    (*parameters).push(launcher::GameOption::new_single(name));
                }
                parameters.push(launcher::GameOption::new_pair("--width".to_owned(), self.parse_token("${resolution_width}", s)));
                parameters.push(launcher::GameOption::new_pair("--height".to_owned(), self.parse_token("${resolution_height}", s)));
            }
            None => if let Some(ref inherits_from) = self.inherits_from {
                let version = manager.version_of(&inherits_from)?;
                return version.collect_game_arguments(manager, parameters, s);
            }
        }
        Result::Ok(())
    }

    pub fn collect_jvm_arguments(&self,
                                 _: &VersionManager,
                                 parameters: &mut Vec<launcher::JvmOption>,
                                 s: &parsing::ParameterStrategy) -> Result<(), Error> {
        if OS_PLATFORM == "windows" { parameters.push(launcher::JvmOption::new("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".to_owned())); }
        parameters.push(launcher::JvmOption::new(self.parse_token("-Djava.library.path=${natives_directory}", s)));
        parameters.push(launcher::JvmOption::new(self.parse_token("-Dminecraft.arguments.brand=${arguments_name}", s)));
        parameters.push(launcher::JvmOption::new(self.parse_token("-Dminecraft.arguments.version=${arguments_version}", s)));
        parameters.push(launcher::JvmOption::new(self.parse_token("-Dminecraft.client.jar=${primary_jar}", s)));
        parameters.push(launcher::JvmOption::new("-cp".to_owned()));
        parameters.push(launcher::JvmOption::new(self.parse_token("${classpath}", s)));
        Result::Ok(())
    }

    pub fn classpath(&self,
                     library_path: &Path,
                     manager: &VersionManager) -> Result<String, Error> {
        self.classpath_with_separator(library_path, CLASSPATH_SEPARATOR, manager)
    }

    pub fn classpath_with_separator(&self,
                                    library_path: &Path,
                                    classpath_separator: &str,
                                    manager: &VersionManager) -> Result<String, Error> {
        let libs = self.libraries(manager)?;
        let mut result = String::new();
        for lib in libs.iter() {
            if !lib.is_native() {
                if let Some(path_buf) = lib.classpath_default(library_path) {
                    let path = fs::canonicalize(path_buf.as_path())?.into_os_string();
                    result.push_str(&path.into_string()?);
                    result.push_str(classpath_separator);
                }
            }
        }
        let primary_jar_path = manager.get_primary_jar_path(self.id.as_str()).into_os_string();
        result.push_str(primary_jar_path.into_string()?.as_str());
        Result::Ok(result)
    }

    pub fn to_native_collection(&self,
                                manager: &VersionManager,
                                library_path: &Path) -> Result<NativeCollection, Error> {
        let mut collection = NativeCollection { libraries: Vec::new() };
        for lib in self.libraries(manager)?.iter() {
            if lib.is_native() {
                if let Some(path_buf) = lib.classpath_default(library_path) {
                    collection.libraries.push((path_buf, lib.extract_ignored.clone()))
                }
            }
        }
        Result::Ok(collection)
    }

    fn parse_token(&self, token: &str, s: &parsing::ParameterStrategy) -> String {
        match parsing::parse(token, s).next() {
            Some(parsed_token) => parsed_token,
            None => token.to_owned()
        }
    }
}

impl AssetDownloadInfo {
    pub fn new(id: String) -> AssetDownloadInfo {
        AssetDownloadInfo {
            size: None,
            url: None,
            sha1: None,
            asset_index_id: id,
            total_size: None,
            size_and_hash_known: false,
        }
    }

    pub fn id(&self) -> &str {
        &self.asset_index_id
    }
}

impl From<AssetDownloadInfo> for DownloadInfo {
    fn from(info: AssetDownloadInfo) -> Self {
        let id = info.asset_index_id;
        match (info.size, info.url, info.sha1, info.size_and_hash_known) {
            (Some(size), Some(url), Some(sha1), true) => DownloadInfo::PreHashed { size, url, sha1 },
            (_, Some(url), _, _) => DownloadInfo::Raw { url },
            _ => DownloadInfo::Raw {
                url: format!("https://s3.amazonaws.com/Minecraft.Download/indexes/{}.json", id),
            }
        }
    }
}

impl DownloadStrategy {
    fn get<'a>(&'a self, arg: &str) -> Option<(&'a str, &'a DownloadInfo)> {
        let mut allowed = self.rules.is_empty();
        for &(ref action, ref os) in &self.rules {
            match action.as_str() {
                "allow" => allowed = os.is_empty() || os == OS_PLATFORM,
                "disallow" => allowed = !os.is_empty() && os != OS_PLATFORM,
                _ => () // just ignore it
            }
        }
        if allowed {
            match self.with_classifier.get(arg) {
                Some(&(ref classifier, ref info)) => Some((&classifier, &info)),
                None => self.default.as_ref().map(|v| ("", v))
            }
        } else {
            None
        }
    }
}

impl Library {
    pub fn is_native(&self) -> bool {
        self.is_native
    }

    pub fn download_info_default(&self) -> Option<&DownloadInfo> {
        self.download_info_of(OS_ARCH, OS_PLATFORM)
    }

    pub fn download_info_of(&self, arch: &str, platform: &str) -> Option<&DownloadInfo> {
        match self.downloads.as_ref().get(&format!("{}bit {}", arch, platform)) {
            Some(ref info) => Some(info.1),
            None => None
        }
    }

    pub fn classpath_default(&self, path: &Path) -> Option<PathBuf> {
        self.classpath_of(path, OS_ARCH, OS_PLATFORM)
    }

    pub fn classpath_of(&self, path: &Path, arch: &str, platform: &str) -> Option<PathBuf> {
        match self.downloads.as_ref().get(&format!("{}bit {}", arch, platform)) {
            Some(ref info) => match Library::get_url_suffix(&self.name, info.0, false) {
                Some(suffix) => {
                    let mut path_buf = path.to_path_buf();
                    path_buf.push(suffix);
                    Some(path_buf)
                }
                None => None
            }
            None => None
        }
    }

    fn get_as_result<E: de::Error>(v: &Value, expected: &str) -> Result<String, E> {
        v.as_str().map(String::from).ok_or_else(|| {
            de::Error::invalid_type(de::Unexpected::UnitVariant, &expected)
        })
    }

    fn get_url_suffix(name: &str, classifier: &str, is_xz: bool) -> Option<String> {
        let parts: Vec<_> = name.splitn(3, ':').collect();
        if parts.len() != 3 { None } else {
            let suffix = if is_xz { "jar.pack.xz" } else { "jar" };
            let dir = format!("{}/{}/{}", parts[0].replace(".", "/"), parts[1], parts[2]);
            if classifier.is_empty() {
                Some(format!("{}/{}-{}.{}", dir, parts[1], parts[2], suffix))
            } else {
                Some(format!("{}/{}-{}-{}.{}", dir, parts[1], parts[2], classifier, suffix))
            }
        }
    }

    fn deserialize_map<'de, A>(mut map: A) -> Result<Library, A::Error> where A: MapAccess<'de> {
        let mut is_xz = false;
        let mut url_prefix: String = String::new();
        let mut natives: HashMap<String, String> = HashMap::new();
        let mut downloads: Value = Value::Null;
        let mut name = String::new();
        let mut extract_ignored = Vec::new();
        let mut library_downloads = DownloadStrategy {
            with_classifier: HashMap::new(),
            rules: Vec::new(),
            default: None,
        };
        while let Some((key, value)) = map.next_entry::<String, Value>()? {
            match key.as_str() {
                "name" => name = Library::get_as_result(&value, "library name")?,
                "url" => url_prefix = Library::get_as_result(&value, "library url prefix")?,
                "checksums" => is_xz = value.is_array(),
                "extract" => if let Some(extract_rules) = value.as_object().and_then(|o| {
                    o.get("exclude").and_then(|v| v.as_array())
                }) {
                    for v in extract_rules.iter() {
                        let rule = Library::get_as_result(v, "extract rules")?;
                        extract_ignored.push(rule);
                    }
                }
                "natives" => if let Some(map) = value.as_object() {
                    for (k, v) in map.iter() {
                        let classifier = Library::get_as_result(v, "os classifier")?;
                        natives.insert(k.clone(), classifier);
                    }
                }
                "rules" => if let Some(list) = value.as_array() {
                    for v in list {
                        if let Some(map) = v.as_object() {
                            if let Some(value) = map.get("action") {
                                let action = Library::get_as_result(value, "rule action")?;
                                if let Some(os) = map.get("os").and_then(|v| {
                                    v.as_object().and_then(|v| v.get("name"))
                                }).map(|v| Library::get_as_result(v, "rule os")) {
                                    library_downloads.rules.push((action, os?));
                                } else {
                                    library_downloads.rules.push((action, String::new()));
                                }
                            }
                        }
                    }
                }
                "downloads" => if value.is_object() {
                    downloads = value.clone();
                }
                _ => () // just ignore it
            }
        }
        if name.is_empty() {
            let err = de::Error::invalid_type(de::Unexpected::UnitVariant, &"library name");
            return Result::Err(err);
        }
        if url_prefix.is_empty() {
            if let Some(map) = downloads.as_object() {
                if let Some(classifiers) = map.get("classifiers").and_then(|v| v.as_object()) {
                    for (os, classifier) in natives.iter() {
                        let classifier_32 = classifier.replace("${arch}", "32");
                        let classifier_64 = classifier.replace("${arch}", "64");
                        if let Some(download_info) = classifiers.get(&classifier_32).and_then(|v| {
                            serde_json::from_value::<DownloadInfo>(v.clone()).ok()
                        }) {
                            let key = format!("32bit {}", os);
                            library_downloads.with_classifier.insert(key, (classifier_32, download_info));
                        }
                        if let Some(download_info) = classifiers.get(&classifier_64).and_then(|v| {
                            serde_json::from_value::<DownloadInfo>(v.clone()).ok()
                        }) {
                            let key = format!("64bit {}", os);
                            library_downloads.with_classifier.insert(key, (classifier_64, download_info));
                        }
                    }
                }
                if let Some(download_info) = map.get("artifact").and_then(|v| {
                    serde_json::from_value::<DownloadInfo>(v.clone()).ok()
                }) {
                    library_downloads.default = Some(download_info);
                }
                return Result::Ok(Library {
                    name,
                    is_native: !natives.is_empty(),
                    downloads: Rc::new(library_downloads),
                    extract_ignored: Rc::new(extract_ignored),
                });
            }
            url_prefix.push_str("https://libraries.minecraft.net/");
        }
        if natives.is_empty() {
            if let Some(suffix) = Library::get_url_suffix(&name, "", is_xz) {
                library_downloads.default = Some(if is_xz {
                    DownloadInfo::RawXzip { url: format!("{}{}", url_prefix, suffix) }
                } else {
                    DownloadInfo::Raw { url: format!("{}{}", url_prefix, suffix) }
                });
            }
        } else {
            for (os, classifier) in natives.iter() {
                let classifier_32 = classifier.replace("${arch}", "32");
                let classifier_64 = classifier.replace("${arch}", "64");
                if let Some(suffix) = Library::get_url_suffix(&name, classifier_32.as_str(), is_xz) {
                    library_downloads.with_classifier.insert(format!("32bit {}", os), (classifier_32, if is_xz {
                        DownloadInfo::RawXzip { url: format!("{}{}", url_prefix, suffix) }
                    } else {
                        DownloadInfo::Raw { url: format!("{}{}", url_prefix, suffix) }
                    }));
                }
                if let Some(suffix) = Library::get_url_suffix(&name, classifier_64.as_str(), is_xz) {
                    library_downloads.with_classifier.insert(format!("64bit {}", os), (classifier_64, if is_xz {
                        DownloadInfo::RawXzip { url: format!("{}{}", url_prefix, suffix) }
                    } else {
                        DownloadInfo::Raw { url: format!("{}{}", url_prefix, suffix) }
                    }));
                }
            }
        }
        Result::Ok(Library {
            name,
            is_native: !natives.is_empty(),
            downloads: Rc::new(library_downloads),
            extract_ignored: Rc::new(extract_ignored),
        })
    }
}

impl<'de> Deserialize<'de> for Library {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct LibraryVisitor;

        impl<'de> Visitor<'de> for LibraryVisitor {
            fn visit_map<A>(self, map: A) -> Result<Library, A::Error> where A: MapAccess<'de> {
                Library::deserialize_map(map)
            }

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("minecraft libraries")
            }

            type Value = Library;
        }

        deserializer.deserialize_map(LibraryVisitor)
    }
}
