use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MailFoldersResponse {
    /// The OData context URL
    #[serde(rename = "@odata.context")]
    pub odata_context: String,

    /// The list of mail folders
    #[serde(rename = "value")]
    pub value: Vec<MailFolder>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MailFolder {
    /// The unique identifier of the mail folder (opaque string from Graph API)
    #[serde(rename = "id")]
    pub id: String,

    /// The display name of the mail folder
    #[serde(rename = "displayName")]
    pub display_name: String,

    /// The ID of the parent folder, if any
    #[serde(rename = "parentFolderId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_folder_id: Option<String>,

    /// Indicates whether the folder is hidden in the client UI
    #[serde(rename = "isHidden")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_hidden: Option<bool>,

    /// The size of the folder in bytes
    #[serde(rename = "sizeInBytes")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_in_bytes: Option<u64>,

    /// Total number of items in the folder
    #[serde(rename = "totalItemCount")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_item_count: Option<u32>,

    /// Number of unread items in the folder
    #[serde(rename = "unreadItemCount")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unread_item_count: Option<u32>,

    /// Number of child folders inside this folder
    #[serde(rename = "childFolderCount")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_folder_count: Option<u32>,
}
