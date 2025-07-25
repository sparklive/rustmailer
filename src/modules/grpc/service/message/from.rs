use crate::modules::{
    cache::imap::{
        envelope::{EmailEnvelope, Received},
        mailbox::{EmailFlag, EnvelopeFlag},
    },
    common::Addr,
    grpc::service::rustmailer_grpc::{self, PagedMessages},
    imap::section::{EmailBodyPart, Encoding, ImapAttachment, Param, PartType, SegmentPath},
    message::{
        attachment::AttachmentRequest,
        content::{MessageContent, MessageContentRequest, PlainText},
        copy::MailboxTransferRequest,
        delete::MessageDeleteRequest,
        flag::{FlagAction, FlagMessageRequest},
        search::payload::{
            Condition, Conditions, Logic, MessageSearch, MessageSearchRequest, Operator,
            UnifiedSearchRequest,
        },
    },
    rest::response::DataPage,
};

impl From<rustmailer_grpc::MailboxTransferRequest> for MailboxTransferRequest {
    fn from(value: rustmailer_grpc::MailboxTransferRequest) -> Self {
        Self {
            uids: value.uids,
            current_mailbox: value.current_mailbox,
            target_mailbox: value.target_mailbox,
        }
    }
}

impl From<rustmailer_grpc::MessageDeleteRequest> for MessageDeleteRequest {
    fn from(value: rustmailer_grpc::MessageDeleteRequest) -> Self {
        Self {
            uids: value.uids,
            mailbox: value.mailbox_name,
        }
    }
}

impl TryFrom<i32> for EmailFlag {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(EmailFlag::Seen),
            1 => Ok(EmailFlag::Answered),
            2 => Ok(EmailFlag::Flagged),
            3 => Ok(EmailFlag::Deleted),
            4 => Ok(EmailFlag::Draft),
            5 => Ok(EmailFlag::Recent),
            6 => Ok(EmailFlag::MayCreate),
            7 => Ok(EmailFlag::Custom),
            _ => Err("Invalid value for EmailFlag"),
        }
    }
}

impl TryFrom<rustmailer_grpc::EnvelopeFlag> for EnvelopeFlag {
    type Error = &'static str;

    fn try_from(value: rustmailer_grpc::EnvelopeFlag) -> Result<Self, Self::Error> {
        Ok(Self {
            flag: EmailFlag::try_from(value.flag)?,
            custom: value.custom,
        })
    }
}

impl TryFrom<rustmailer_grpc::FlagAction> for FlagAction {
    type Error = &'static str;

    fn try_from(value: rustmailer_grpc::FlagAction) -> Result<Self, Self::Error> {
        Ok(Self {
            add: if value.add.is_empty() {
                None
            } else {
                Some(
                    value
                        .add
                        .into_iter()
                        .map(EnvelopeFlag::try_from)
                        .collect::<Result<Vec<EnvelopeFlag>, _>>()?,
                )
            },
            remove: if value.remove.is_empty() {
                None
            } else {
                Some(
                    value
                        .remove
                        .into_iter()
                        .map(EnvelopeFlag::try_from)
                        .collect::<Result<Vec<EnvelopeFlag>, _>>()?,
                )
            },
            overwrite: if value.overwrite.is_empty() {
                None
            } else {
                Some(
                    value
                        .overwrite
                        .into_iter()
                        .map(EnvelopeFlag::try_from)
                        .collect::<Result<Vec<EnvelopeFlag>, _>>()?,
                )
            },
        })
    }
}

impl TryFrom<rustmailer_grpc::FlagMessageRequest> for FlagMessageRequest {
    type Error = &'static str;

    fn try_from(value: rustmailer_grpc::FlagMessageRequest) -> Result<Self, Self::Error> {
        let action = value
            .action
            .ok_or("field 'action' is misssing")?
            .try_into()?;
        Ok(Self {
            uids: value.uids,
            mailbox: value.mailbox_name,
            action,
        })
    }
}

impl From<DataPage<EmailEnvelope>> for PagedMessages {
    fn from(value: DataPage<EmailEnvelope>) -> Self {
        Self {
            current_page: value.current_page,
            page_size: value.page_size,
            total_items: value.total_items,
            items: value.items.into_iter().map(Into::into).collect(),
            total_pages: value.total_pages,
        }
    }
}

impl From<EmailEnvelope> for rustmailer_grpc::EmailEnvelope {
    fn from(value: EmailEnvelope) -> Self {
        Self {
            account_id: value.account_id,
            mailbox_id: value.mailbox_id,
            mailbox_name: value.mailbox_name,
            uid: value.uid,
            internal_date: value.internal_date,
            size: value.size,
            flags: value.flags.into_iter().map(Into::into).collect(),
            flags_hash: value.flags_hash,
            bcc: value
                .bcc
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            cc: value
                .cc
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            date: value.date,
            from: value.from.map(Into::into),
            in_reply_to: value.in_reply_to,
            sender: value.sender.map(Into::into),
            return_address: value.return_address,
            message_id: value.message_id,
            subject: value.subject,
            thread_name: value.thread_name,
            mime_version: value.mime_version,
            references: value.references.unwrap_or_default(),
            reply_to: value
                .reply_to
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            to: value
                .to
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            attachments: value
                .attachments
                .into_iter()
                .flatten()
                .map(Into::into)
                .collect(),
            body_meta: value
                .body_meta
                .into_iter()
                .flatten()
                .map(Into::into)
                .collect(),
            received: value.received.map(Into::into),
        }
    }
}

impl From<SegmentPath> for rustmailer_grpc::SegmentPath {
    fn from(value: SegmentPath) -> Self {
        Self {
            segments: value.segments,
        }
    }
}

impl From<rustmailer_grpc::SegmentPath> for SegmentPath {
    fn from(value: rustmailer_grpc::SegmentPath) -> Self {
        Self {
            segments: value.segments,
        }
    }
}

impl From<Encoding> for i32 {
    fn from(value: Encoding) -> Self {
        match value {
            Encoding::None => 0,
            Encoding::QuotedPrintable => 1,
            Encoding::Base64 => 2,
        }
    }
}

impl TryFrom<i32> for Encoding {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Encoding::None),
            1 => Ok(Encoding::QuotedPrintable),
            2 => Ok(Encoding::Base64),
            _ => Err("Invalid value for Encoding"),
        }
    }
}

impl From<ImapAttachment> for rustmailer_grpc::ImapAttachment {
    fn from(value: ImapAttachment) -> Self {
        Self {
            id: value.id,
            path: Some(value.path.into()),
            filename: value.filename,
            inline: value.inline,
            content_id: value.content_id,
            size: value.size as u64,
            file_type: value.file_type,
            transfer_encoding: value.transfer_encoding.into(),
        }
    }
}

impl TryFrom<rustmailer_grpc::ImapAttachment> for ImapAttachment {
    type Error = &'static str;

    fn try_from(value: rustmailer_grpc::ImapAttachment) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            path: value.path.ok_or("field 'path' is missing")?.into(),
            filename: value.filename,
            inline: value.inline,
            content_id: value.content_id,
            size: value.size as usize,
            file_type: value.file_type,
            transfer_encoding: value.transfer_encoding.try_into()?,
        })
    }
}

impl From<PartType> for i32 {
    fn from(value: PartType) -> Self {
        match value {
            PartType::Plain => 0,
            PartType::Html => 1,
        }
    }
}

impl TryFrom<i32> for PartType {
    type Error = &'static str;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(PartType::Plain),
            1 => Ok(PartType::Html),
            _ => Err("Invalid value for PartType"),
        }
    }
}

impl From<Param> for rustmailer_grpc::Param {
    fn from(value: Param) -> Self {
        Self {
            key: value.key,
            value: value.value,
        }
    }
}

impl From<rustmailer_grpc::Param> for Param {
    fn from(value: rustmailer_grpc::Param) -> Self {
        Self {
            key: value.key,
            value: value.value,
        }
    }
}

impl From<EmailBodyPart> for rustmailer_grpc::EmailBodyPart {
    fn from(value: EmailBodyPart) -> Self {
        Self {
            id: value.id,
            part_type: value.part_type.into(),
            path: Some(value.path.into()),
            params: value.params.into_iter().flatten().map(Into::into).collect(),
            size: value.size as u64,
            transfer_encoding: value.transfer_encoding.into(),
        }
    }
}

impl TryFrom<rustmailer_grpc::EmailBodyPart> for EmailBodyPart {
    type Error = &'static str;

    fn try_from(value: rustmailer_grpc::EmailBodyPart) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            part_type: value.part_type.try_into()?,
            path: value.path.ok_or("field 'path' missing")?.into(),
            params: {
                (!value.params.is_empty())
                    .then(|| value.params.into_iter().map(Into::into).collect())
            },
            size: value.size as usize,
            transfer_encoding: value.transfer_encoding.try_into()?,
        })
    }
}

impl From<Addr> for rustmailer_grpc::Addr {
    fn from(value: Addr) -> Self {
        Self {
            name: value.name,
            address: value.address,
        }
    }
}

impl From<Received> for rustmailer_grpc::Received {
    fn from(value: Received) -> Self {
        Self {
            from: value.from,
            by: value.by,
            with: value.with,
            date: value.date,
        }
    }
}

impl TryFrom<rustmailer_grpc::FetchMessageContentRequest> for MessageContentRequest {
    type Error = &'static str;

    fn try_from(value: rustmailer_grpc::FetchMessageContentRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            mailbox: value.mailbox_name,
            uid: value.uid,
            max_length: value.max_length.map(|m| m as usize),
            sections: value
                .sections
                .into_iter()
                .map(EmailBodyPart::try_from)
                .collect::<Result<Vec<EmailBodyPart>, _>>()?,
            inline: (!value.inline.is_empty())
                .then(|| {
                    value
                        .inline
                        .into_iter()
                        .map(ImapAttachment::try_from)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?,
        })
    }
}

impl From<MessageContent> for rustmailer_grpc::MessageContentResponse {
    fn from(value: MessageContent) -> Self {
        Self {
            plain: value.plain.map(Into::into),
            html: value.html,
        }
    }
}

impl From<PlainText> for rustmailer_grpc::PlainText {
    fn from(value: PlainText) -> Self {
        Self {
            content: value.content,
            truncated: value.truncated,
        }
    }
}

impl TryFrom<rustmailer_grpc::FetchMessageAttachmentRequest> for AttachmentRequest {
    type Error = &'static str;

    fn try_from(
        value: rustmailer_grpc::FetchMessageAttachmentRequest,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            uid: value.uid,
            mailbox: value.mailbox_name,
            attachment: value
                .attachment
                .ok_or("field 'attachment' is missing")?
                .try_into()?,
        })
    }
}

impl TryFrom<rustmailer_grpc::MessageSearch> for MessageSearch {
    type Error = &'static str;

    fn try_from(value: rustmailer_grpc::MessageSearch) -> Result<Self, Self::Error> {
        let search_type = value.search_type.ok_or("field 'SearchType' is Missing")?;

        match search_type {
            rustmailer_grpc::message_search::SearchType::Condition(condition) => {
                Ok(MessageSearch::Condition(condition.try_into()?))
            }
            rustmailer_grpc::message_search::SearchType::Logic(logic) => {
                Ok(MessageSearch::Logic(logic.try_into()?))
            }
        }
    }
}

impl From<rustmailer_grpc::UnifiedSearchRequest> for UnifiedSearchRequest {
    fn from(value: rustmailer_grpc::UnifiedSearchRequest) -> Self {
        Self {
            accounts: if value.accounts.is_empty() {
                None
            } else {
                Some(value.accounts)
            },
            email: value.email,
            after: value.after,
            before: value.before,
        }
    }
}

impl TryFrom<rustmailer_grpc::Logic> for Logic {
    type Error = &'static str;

    fn try_from(value: rustmailer_grpc::Logic) -> Result<Self, Self::Error> {
        Ok(Logic {
            operator: value.operator.try_into()?,
            children: value
                .children
                .into_iter()
                .map(MessageSearch::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl TryFrom<i32> for Operator {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Operator::And),
            1 => Ok(Operator::Or),
            2 => Ok(Operator::Not),
            _ => Err("Invalid value for Operator"),
        }
    }
}

impl TryFrom<rustmailer_grpc::Condition> for Condition {
    type Error = &'static str;

    fn try_from(value: rustmailer_grpc::Condition) -> Result<Self, Self::Error> {
        Ok(Self {
            condition: value.condition.try_into()?,
            value: value.value,
        })
    }
}

impl TryFrom<i32> for Conditions {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Conditions::All),
            1 => Ok(Conditions::Answered),
            2 => Ok(Conditions::Bcc),
            3 => Ok(Conditions::Before),
            4 => Ok(Conditions::Body),
            5 => Ok(Conditions::Cc),
            6 => Ok(Conditions::Deleted),
            7 => Ok(Conditions::Draft),
            8 => Ok(Conditions::Flagged),
            9 => Ok(Conditions::From),
            10 => Ok(Conditions::Header),
            11 => Ok(Conditions::Keyword),
            12 => Ok(Conditions::Larger),
            13 => Ok(Conditions::New),
            14 => Ok(Conditions::Old),
            15 => Ok(Conditions::On),
            16 => Ok(Conditions::Recent),
            17 => Ok(Conditions::Seen),
            18 => Ok(Conditions::SentBefore),
            19 => Ok(Conditions::SentOn),
            20 => Ok(Conditions::SentSince),
            21 => Ok(Conditions::Since),
            22 => Ok(Conditions::Smaller),
            23 => Ok(Conditions::Subject),
            24 => Ok(Conditions::Text),
            25 => Ok(Conditions::To),
            26 => Ok(Conditions::Uid),
            27 => Ok(Conditions::Unanswered),
            28 => Ok(Conditions::Undeleted),
            29 => Ok(Conditions::Undraft),
            30 => Ok(Conditions::Unflagged),
            31 => Ok(Conditions::Unkeyword),
            32 => Ok(Conditions::Unseen),
            _ => Err("Invalid value for Conditions"),
        }
    }
}

impl TryFrom<rustmailer_grpc::MessageSearchRequest> for MessageSearchRequest {
    type Error = &'static str;

    fn try_from(value: rustmailer_grpc::MessageSearchRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            search: value.search.ok_or("field 'search' Missing")?.try_into()?,
            mailbox: value.mailbox_name,
        })
    }
}
