use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use url::Url;

pub static BASE_URL: Lazy<Url> = Lazy::new(|| Url::parse("https://slack.com/api/").unwrap());

pub const RTM_CONNECT: &str = "rtm.connect";

#[derive(Debug, Default, Serialize)]
pub struct RtmConnect {
    pub token: String,
    pub batch_presence_aware: Option<i32>,
    pub presence_sub: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ConnectResponse {
    pub ok: bool,
    #[serde(rename = "self")]
    pub self_: ConnectSelf,
    pub team: ConnectTeam,
    pub url: Url,
}

#[derive(Debug, Deserialize)]
pub struct ConnectSelf {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ConnectTeam {
    pub domain: String,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Event {
    Hello,
    Error {
        code: i32,
        msg: String,
    },
    Message {
        channel: String,
        user: String,
        text: String,
        ts: String,
        edited: Option<Edited>,
        subtype: Option<String>,
    },
    AccountsChanged,
    BotAdded,
    BotChanged,
    ChannelArchive,
    ChannelCreated,
    ChannelDeleted,
    ChannelHistoryChanged,
    ChannelJoined,
    ChannelLeft,
    ChannelMarked,
    ChannelRename,
    ChannelUnarchive,
    CommandsChanged,
    DndUpdated,
    DndUpdatedUser,
    EmailDomainChanged,
    EmojiChanged,
    ExternalOrgMigrationFinished,
    ExternalOrgMigrationStarted,
    FileChange,
    FileCommentAdded,
    FileCommentDeleted,
    FileCommentEdited,
    FileCreated,
    FileDeleted,
    FilePublic {
        file_id: String,
        file: File,
    },
    FileShared {
        file_id: String,
        file: File,
    },
    FileUnshared,
    Goodbye,
    GroupArchive,
    GroupClose,
    GroupDeleted,
    GroupHistoryChanged,
    GroupJoined,
    GroupLeft,
    GroupMarked,
    GroupOpen,
    GroupRename,
    GroupUnarchive,
    ImClose,
    ImCreated,
    ImHistoryChanged,
    ImMarked,
    ImOpen,
    ManualPresenceChange,
    MemberJoinedChannel,
    MemberLeftChannel,
    PinAdded,
    PinRemoved,
    PrefChange,
    PresenceChange,
    PresenceQuery,
    PresenceSub,
    ReactionAdded,
    ReactionRemoved,
    ReconnectUrl,
    StarAdded,
    StarRemoved,
    SubteamCreated,
    SubteamMembersChanged,
    SubteamSelfAdded,
    SubteamSelfRemoved,
    SubteamUpdated,
    TeamDomainChange,
    TeamJoin,
    TeamMigrationStarted,
    TeamPlanChange,
    TeamPrefChange,
    TeamProfileChange,
    TeamProfileDelete,
    TeamProfileReorder,
    TeamRename,
    UserChange,
    UserTyping,
}

#[derive(Debug, Deserialize)]
pub struct Edited {
    pub user: String,
    pub ts: String,
}

#[derive(Debug, Deserialize)]
pub struct File {
    id: String,
}
