use std::time::Duration;

const GRAPH_ROOT: &str = "https://graph.microsoft.com/v1.0";
const SELECT: &str = "$select=name,size,lastModifiedDateTime,file,folder";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudScope {
    Full,
    Folder(String),
    File(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudFile {
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified: String,
    pub quickxor: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Debug)]
pub enum FetchError {
    NotFound,
    Other(String),
}

pub fn drive_relative(cloud_path: &str) -> String {
    let trimmed = cloud_path.trim().trim_matches(['/', '\\']).replace('\\', "/");
    if trimmed.eq_ignore_ascii_case("OneDrive") {
        return String::new();
    }
    let rest = trimmed
        .strip_prefix("OneDrive/")
        .or_else(|| trimmed.strip_prefix("onedrive/"))
        .unwrap_or(trimmed.as_str());
    rest.trim_matches('/').to_string()
}

pub fn children_url(drive_path: &str) -> Result<String, String> {
    let url = if drive_path.is_empty() {
        format!("{GRAPH_ROOT}/me/drive/root/children?{SELECT}")
    } else {
        format!(
            "{GRAPH_ROOT}/me/drive/root:/{encoded}:/children?{SELECT}",
            encoded = encode_path(drive_path)
        )
    };
    assert_metadata_url(&url)?;
    Ok(url)
}

pub fn item_url(drive_path: &str) -> Result<String, String> {
    if drive_path.is_empty() {
        return Err("cloud file path is empty".to_string());
    }
    let url = format!(
        "{GRAPH_ROOT}/me/drive/root:/{encoded}:?{SELECT}",
        encoded = encode_path(drive_path)
    );
    assert_metadata_url(&url)?;
    Ok(url)
}

pub fn assert_metadata_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://graph.microsoft.com/") {
        return Err("OneDrive metadata URL must stay on graph.microsoft.com".to_string());
    }
    if url.to_ascii_lowercase().contains("/content") {
        return Err("OneDrive file contents are not downloaded".to_string());
    }
    Ok(())
}

pub fn list_files(
    cloud_path: &str,
    scope: &CloudScope,
    token: &str,
    on_folder: impl FnMut(&str),
) -> Result<Vec<CloudFile>, String> {
    list_files_with(cloud_path, scope, |url| http_get(url, token), on_folder)
}

pub fn list_files_with(
    cloud_path: &str,
    scope: &CloudScope,
    mut fetch: impl FnMut(&str) -> Result<String, FetchError>,
    mut on_folder: impl FnMut(&str),
) -> Result<Vec<CloudFile>, String> {
    let root = drive_relative(cloud_path);
    match scope {
        CloudScope::File(relative) => {
            let path = join_path(&root, relative);
            on_folder(&path);
            let url = item_url(&path)?;
            match fetch(&url) {
                Ok(body) => parse_item(&body, relative).map(|file| file.into_iter().collect()),
                Err(FetchError::NotFound) => Ok(Vec::new()),
                Err(FetchError::Other(message)) => Err(message),
            }
        }
        CloudScope::Folder(folder) => {
            let path = join_path(&root, folder);
            let mut files = Vec::new();
            walk_children(&path, folder, &mut fetch, &mut on_folder, &mut files, 0)?;
            Ok(files)
        }
        CloudScope::Full => {
            let mut files = Vec::new();
            walk_children(&root, "", &mut fetch, &mut on_folder, &mut files, 0)?;
            Ok(files)
        }
    }
}

fn walk_children(
    drive_path: &str,
    relative_prefix: &str,
    fetch: &mut impl FnMut(&str) -> Result<String, FetchError>,
    on_folder: &mut impl FnMut(&str),
    files: &mut Vec<CloudFile>,
    depth: u8,
) -> Result<(), String> {
    if depth > 64 {
        return Err("OneDrive folder is nested too deeply".to_string());
    }
    on_folder(if drive_path.is_empty() {
        "OneDrive"
    } else {
        drive_path
    });
    let mut next = Some(children_url(drive_path)?);
    while let Some(url) = next {
        assert_metadata_url(&url)?;
        let body = match fetch(&url) {
            Ok(body) => body,
            Err(FetchError::NotFound) => return Ok(()),
            Err(FetchError::Other(message)) => return Err(message),
        };
        let page = parse_page(&body)?;
        for item in page.items {
            if !safe_name(&item.name) {
                continue;
            }
            let relative = if relative_prefix.is_empty() {
                item.name.clone()
            } else {
                format!("{relative_prefix}/{}", item.name)
            };
            if item.is_folder {
                let child_path = join_path(drive_path, &item.name);
                walk_children(&child_path, &relative, fetch, on_folder, files, depth + 1)?;
            } else {
                files.push(CloudFile {
                    relative_path: relative,
                    size_bytes: item.size_bytes,
                    modified: item.modified,
                    quickxor: item.quickxor,
                    sha256: item.sha256,
                });
            }
        }
        next = match page.next_link {
            Some(link) => {
                assert_metadata_url(&link)?;
                Some(link)
            }
            None => None,
        };
    }
    Ok(())
}

pub(crate) struct Page {
    items: Vec<Item>,
    next_link: Option<String>,
}

struct Item {
    name: String,
    size_bytes: u64,
    modified: String,
    is_folder: bool,
    quickxor: Option<String>,
    sha256: Option<String>,
}

pub fn parse_page(body: &str) -> Result<Page, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|err| format!("OneDrive metadata was not JSON: {err}"))?;
    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(|item| item.as_str())
            .unwrap_or("OneDrive metadata request failed");
        return Err(message.to_string());
    }
    let items = value
        .get("value")
        .and_then(|item| item.as_array())
        .map(|items| items.iter().filter_map(parse_drive_item).collect())
        .unwrap_or_default();
    let next_link = value
        .get("@odata.nextLink")
        .and_then(|item| item.as_str())
        .map(str::to_string);
    Ok(Page { items, next_link })
}

fn parse_item(body: &str, relative: &str) -> Result<Option<CloudFile>, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|err| format!("OneDrive metadata was not JSON: {err}"))?;
    if value.get("error").is_some() {
        return Ok(None);
    }
    let Some(item) = parse_drive_item(&value) else {
        return Ok(None);
    };
    if item.is_folder {
        return Err(format!("{relative} is a folder"));
    }
    Ok(Some(CloudFile {
        relative_path: relative.to_string(),
        size_bytes: item.size_bytes,
        modified: item.modified,
        quickxor: item.quickxor,
        sha256: item.sha256,
    }))
}

fn parse_drive_item(value: &serde_json::Value) -> Option<Item> {
    let name = value.get("name")?.as_str()?.to_string();
    let is_folder = value.get("folder").is_some();
    let size_bytes = value.get("size").and_then(|item| item.as_u64()).unwrap_or(0);
    let modified = value
        .get("lastModifiedDateTime")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string();
    let hashes = value.pointer("/file/hashes");
    let quickxor = hashes
        .and_then(|item| item.get("quickXorHash"))
        .and_then(|item| item.as_str())
        .filter(|item| !item.is_empty())
        .map(str::to_string);
    let sha256 = hashes
        .and_then(|item| item.get("sha256Hash"))
        .and_then(|item| item.as_str())
        .filter(|item| !item.is_empty())
        .map(str::to_string);
    Some(Item {
        name,
        size_bytes,
        modified,
        is_folder,
        quickxor,
        sha256,
    })
}

fn http_get(url: &str, token: &str) -> Result<String, FetchError> {
    assert_metadata_url(url).map_err(FetchError::Other)?;
    let response = agent()
        .get(url)
        .set("Authorization", &format!("Bearer {token}"))
        .call();
    match response {
        Ok(response) => response.into_string().map_err(|err| FetchError::Other(err.to_string())),
        Err(ureq::Error::Status(404, _)) => Err(FetchError::NotFound),
        Err(ureq::Error::Status(401, _)) => {
            Err(FetchError::Other("OneDrive token expired".to_string()))
        }
        Err(ureq::Error::Status(code, response)) => {
            let body = response.into_string().unwrap_or_default();
            let snippet: String = body.chars().take(180).collect();
            Err(FetchError::Other(format!("OneDrive metadata {code}: {snippet}")))
        }
        Err(err) => Err(FetchError::Other(format!("OneDrive metadata request failed: {err}"))),
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(45))
        .build()
}

fn join_path(root: &str, child: &str) -> String {
    let child = child.trim_matches('/');
    if root.is_empty() {
        child.to_string()
    } else if child.is_empty() {
        root.to_string()
    } else {
        format!("{root}/{child}")
    }
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_segment(segment: &str) -> String {
    let mut out = String::new();
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn safe_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains('/') && !name.contains('\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_onedrive_prefix() {
        assert_eq!(drive_relative("/OneDrive/Backups/DJI/"), "Backups/DJI");
        assert_eq!(drive_relative("/OneDrive/"), "");
        assert_eq!(drive_relative("Backups/DJI"), "Backups/DJI");
    }

    #[test]
    fn children_url_never_requests_content() {
        let url = children_url("Backups/DJI").unwrap();
        assert!(url.contains("/me/drive/root:/Backups/DJI:/children"));
        assert!(url.contains("quickXorHash") || url.contains("file"));
        assert_metadata_url(&url).unwrap();
        let blocked = assert_metadata_url(
            "https://graph.microsoft.com/v1.0/me/drive/items/1/content",
        );
        assert!(blocked.is_err());
    }

    #[test]
    fn parses_quickxor_without_using_file_bytes() {
        let body = r#"{
            "value": [{
                "name": "clip.txt",
                "size": 12,
                "lastModifiedDateTime": "2026-09-16T00:00:00Z",
                "file": { "hashes": { "quickXorHash": "abc=", "sha256Hash": "zz" } }
            }]
        }"#;
        let page = parse_page(body).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].quickxor.as_deref(), Some("abc="));
        assert_eq!(page.items[0].size_bytes, 12);
    }

    #[test]
    fn walker_requests_children_not_content() {
        let body = r#"{"value":[{"name":"clip.txt","size":4,"lastModifiedDateTime":"2026-09-16T00:00:00Z","file":{"hashes":{"quickXorHash":"qq=="}}}]}"#;
        let mut urls = Vec::new();
        let files = list_files_with(
            "/OneDrive/Backups/DJI/",
            &CloudScope::Full,
            |url| {
                urls.push(url.to_string());
                assert!(!url.contains("/content"));
                Ok(body.to_string())
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "clip.txt");
        assert_eq!(files[0].quickxor.as_deref(), Some("qq=="));
        assert!(urls[0].contains("/children"));
        assert!(!urls.iter().any(|url| url.contains("/content")));
    }
}
