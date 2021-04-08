use matrix_sdk::{
    events::room::message::{MessageEventContent, MessageType},
    identifiers::UserId,
};

pub fn format_message_body<'a>(content: &'a MessageEventContent) -> &'a str {
    use MessageType::*;
    match &content.msgtype {
        Audio(content) => &content.body,
        Emote(content) => &content.body,
        File(content) => &content.body,
        Image(content) => &content.body,
        Location(content) => &content.body,
        Notice(content) => &content.body,
        ServerNotice(content) => &content.body,
        Text(content) => &content.body,
        Video(content) => &content.body,
        VerificationRequest(content) => &content.body,
        _ => "(mient message) plz implement me!",
    }
}

pub fn format_reply_content(
    replied_to_content: &MessageEventContent,
    sender: &UserId,
    reply: &String,
) -> String {
    let quoted_replied = format_message_body(replied_to_content)
        .lines()
        // skip quoted content, those are previous replied_to
        .skip_while(|s| s.starts_with(">"))
        .map(|s| format!("> {}", s))
        .collect::<Vec<String>>()
        .join("\n");
    format!("> <{}> {}\n\n{}", sender, quoted_replied, reply)
}
