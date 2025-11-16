pub struct ChunkInnerPathHelper;

impl ChunkInnerPathHelper {
    pub fn normalize_virtual_path(raw_path: &str) -> String {
        raw_path
            .replace('\\', "/")
            .split('/')
            .filter(|segment| !segment.is_empty() && *segment != ".")
            .collect::<Vec<&str>>()
            .join("/")
    }

    pub fn strip_chunk_suffix(path: &str) -> String {
        if let Some(idx) = path.rfind('/') {
            let suffix = &path[idx + 1..];
            if Self::is_chunk_suffix(suffix) {
                return path[..idx].to_string();
            }
        } else if Self::is_chunk_suffix(path) {
            return String::new();
        }
        path.trim_matches('/').to_string()
    }

    pub fn is_chunk_suffix(segment: &str) -> bool {
        if !segment.contains(':') {
            return false;
        }
        segment
            .chars()
            .all(|c| c.is_ascii_digit() || c == ':' || c == '-')
    }
}
