//! Tool registration: keep routes and names in one place as My Tools grows.

pub struct Tool {
    pub id: &'static str,
    pub label: &'static str,
    pub mobile_path: &'static str,
    pub status_path: &'static str,
    pub upload_path: &'static str,
}

pub const SNAP_TO_PASTE_IMAGE: Tool = Tool {
    id: "snap-to-paste-image",
    label: "Snap to Paste Image",
    mobile_path: "/tools/snap-to-paste-image/",
    status_path: "/api/tools/snap-to-paste-image/status",
    upload_path: "/api/tools/snap-to-paste-image/photo",
};

pub const PROMPT_NOTES: Tool = Tool {
    id: "prompt-notes",
    label: "Prompt Notes",
    mobile_path: "", // Windows-native keyboard shortcut and scratchpad.
    status_path: "",
    upload_path: "",
};

pub const QUICK_NOTE: Tool = Tool {
    id: "quick-note",
    label: "Quick Note",
    mobile_path: "",
    status_path: "",
    upload_path: "",
};

pub const ALL: &[Tool] = &[SNAP_TO_PASTE_IMAGE, PROMPT_NOTES, QUICK_NOTE];

pub fn catalog_json() -> String {
    let tools = ALL
        .iter()
        .map(|tool| {
            if tool.mobile_path.is_empty() {
                format!("{{\"id\":\"{}\",\"label\":\"{}\",\"platform\":\"windows\",\"path\":null}}",
                    tool.id, tool.label)
            } else {
                format!("{{\"id\":\"{}\",\"label\":\"{}\",\"platform\":\"iphone-windows\",\"path\":\"{}\"}}",
                    tool.id, tool.label, tool.mobile_path)
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"tools\":[{tools}]}}")
}

pub fn is_image_status_route(path: &str) -> bool {
    path == SNAP_TO_PASTE_IMAGE.status_path || path == "/api/status"
}

pub fn is_image_upload_route(path: &str) -> bool {
    path == SNAP_TO_PASTE_IMAGE.upload_path || path == "/api/photo"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_routes_are_stable_and_namespaced() {
        assert_eq!(ALL.len(), 3);
        assert_eq!(ALL[0].id, "snap-to-paste-image");
        assert!(is_image_status_route("/api/status"));
        assert!(is_image_status_route(SNAP_TO_PASTE_IMAGE.status_path));
        assert!(is_image_upload_route("/api/photo"));
        assert!(is_image_upload_route(SNAP_TO_PASTE_IMAGE.upload_path));
        assert!(!is_image_upload_route("/api/tools/untrusted/photo"));
        assert!(catalog_json().contains("Snap to Paste Image"));
        assert!(catalog_json().contains("\"id\":\"prompt-notes\""));
        assert!(catalog_json().contains("\"id\":\"quick-note\""));
    }
}
