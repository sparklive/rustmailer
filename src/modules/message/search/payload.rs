use crate::modules::cache::imap::sync::flow::generate_uid_sequence_hashset;
use crate::modules::error::code::ErrorCode;
use crate::modules::message::search::cache::IMAP_SEARCH_CACHE;
use crate::{
    encode_mailbox_name,
    modules::{
        account::entity::Account, cache::imap::envelope::EmailEnvelope,
        context::executors::RUST_MAIL_CONTEXT, envelope::extractor::extract_envelope,
        error::RustMailerResult, rest::response::DataPage,
    },
    raise_error,
};
use chrono::NaiveDate;
use poem_openapi::{Enum, Object, Union};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

/// Represents a single search condition for email messages  
///  
/// A condition consists of a field to search (e.g., FROM, SUBJECT) and an optional value to match.  
/// Some conditions (like ALL, ANSWERED) don't require a value.  
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Object)]
pub struct Condition {
    /// The type of condition to apply (which field or attribute to search)  
    pub condition: Conditions,
    /// The value to search for (may be null for some condition types)  
    pub value: Option<String>,
}

/// Represents a logical expression combining multiple search conditions  
///  
/// This allows for complex searches using logical operators (AND, OR, NOT)  
/// to combine multiple conditions.  
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Object)]
pub struct Logic {
    pub operator: Operator,
    pub children: Vec<MessageSearch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Enum)]
pub enum Operator {
    And,
    Or,
    Not,
}

/// Enumeration of all possible email search condition types  
///  
/// These conditions correspond to standard IMAP search criteria and allow  
/// searching by various email attributes like sender, recipient, subject, date, etc.  
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Enum)]
pub enum Conditions {
    /// Match all messages  
    All,
    /// Messages with the Answered flag set  
    Answered,
    /// Messages with the specified text in the BCC field  
    Bcc,
    /// Messages received before the specified date  
    Before,
    /// Messages with the specified text in the body  
    Body,
    /// Messages with the specified text in the CC field  
    Cc,
    /// Messages with the Deleted flag set  
    Deleted,
    /// Messages with the Draft flag set  
    Draft,
    /// Messages with the Flagged flag set  
    Flagged,
    /// Messages with the specified text in the FROM field  
    From,
    /// Messages with a specific header containing the specified text  
    Header,
    /// Messages with the specified keyword flag set  
    Keyword,
    /// Messages larger than the specified size in bytes  
    Larger,
    /// Messages that are new (recently arrived and not seen)  
    New,
    /// Messages that are old (not recently arrived)  
    Old,
    /// Messages received on the specified date  
    On,
    /// Messages that have been recently delivered  
    Recent,
    /// Messages that have been seen (read)  
    Seen,
    /// Messages sent before the specified date  
    SentBefore,
    /// Messages sent on the specified date  
    SentOn,
    /// Messages sent since the specified date  
    SentSince,
    /// Messages received since the specified date  
    Since,
    /// Messages smaller than the specified size in bytes  
    Smaller,
    /// Messages with the specified text in the subject  
    Subject,
    /// Messages containing the specified text in headers or body  
    Text,
    /// Messages with the specified text in the TO field  
    To,
    /// Messages with the specified UID  
    Uid,
    /// Messages that have not been answered  
    Unanswered,
    /// Messages that have not been deleted  
    Undeleted,
    /// Messages that are not drafts  
    Undraft,
    /// Messages that are not flagged  
    Unflagged,
    /// Messages that don't have the specified keyword flag  
    Unkeyword,
    /// Messages that have not been seen (unread)  
    Unseen,
}

/// Represents search criteria for finding email messages  
///  
/// This enum supports two types of search expressions:  
/// 1. Simple conditions (e.g., "FROM contains example@example.com")  
/// 2. Logical expressions that combine multiple conditions (e.g., "FROM contains X AND SUBJECT contains Y")  
///  
/// Example JSON payload:  
/// ```json  
/// {  
///    "type": "Logic",  
///    "operator": "AND",  
///    "children": [  
///      {  
///        "type": "Condition",  
///        "condition": "FROM",  
///        "value": "example@example.com"  
///      },  
///      {  
///        "type": "Condition",  
///        "condition": "SUBJECT",  
///        "value": "Hello"  
///      }  
///    ]  
/// }  
/// ```  
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Union)]
#[oai(discriminator_name = "type")]
pub enum MessageSearch {
    /// A single search condition  
    Condition(Condition),
    /// A logical expression combining multiple search conditions  
    Logic(Logic),
}
// payload example:
//  {
//     "type": "Logic",
//     "operator": "AND",
//     "children": [
//       {
//         "type": "Condition",
//         "condition": "FROM",
//         "value": "example@example.com"
//       },
//       {
//         "type": "Condition",
//         "condition": "SUBJECT",
//         "value": "Hello"
//       }
//     ]
//  }

impl MessageSearch {
    pub fn to_imap_command(&self) -> RustMailerResult<String> {
        match self {
            Self::Condition(condition) => {
                let c = &condition.condition;
                let value = condition.value.as_deref();

                let command = match c {
                    Conditions::All => "ALL".into(),
                    Conditions::Answered => "ANSWERED".into(),
                    Conditions::Bcc => format!("BCC {}", Self::quote_value(value)?),
                    Conditions::Before => format!("BEFORE {}", Self::format_date(value)?),
                    Conditions::Body => format!("BODY {}", Self::quote_value(value)?),
                    Conditions::Cc => format!("CC {}", Self::quote_value(value)?),
                    Conditions::Deleted => "DELETED".into(),
                    Conditions::Draft => "DRAFT".into(),
                    Conditions::Flagged => "FLAGGED".into(),
                    Conditions::From => format!("FROM {}", Self::quote_value(value)?),
                    Conditions::Header => {
                        let parts: Vec<&str> = value.unwrap_or("").splitn(2, ' ').collect();
                        if parts.len() == 2 {
                            format!(
                                "HEADER {} {}",
                                Self::quote_value(Some(parts[0]))?,
                                Self::quote_value(Some(parts[1]))?
                            )
                        } else {
                            return Err(raise_error!(
                                "Invalid HEADER format (expected 'header_name value')".into(),
                                ErrorCode::InvalidParameter
                            ));
                        }
                    }
                    Conditions::Keyword => format!("KEYWORD {}", Self::quote_value(value)?),
                    Conditions::Larger => format!("LARGER {}", Self::validate_number(value)?),
                    Conditions::New => "NEW".into(),
                    Conditions::Old => "OLD".into(),
                    Conditions::On => format!("ON {}", Self::format_date(value)?),
                    Conditions::Recent => "RECENT".into(),
                    Conditions::Seen => "SEEN".into(),
                    Conditions::SentBefore => format!("SENTBEFORE {}", Self::format_date(value)?),
                    Conditions::SentOn => format!("SENTON {}", Self::format_date(value)?),
                    Conditions::SentSince => format!("SENTSINCE {}", Self::format_date(value)?),
                    Conditions::Since => format!("SINCE {}", Self::format_date(value)?),
                    Conditions::Smaller => format!("SMALLER {}", Self::validate_number(value)?),
                    Conditions::Subject => format!("SUBJECT {}", Self::quote_value(value)?),
                    Conditions::Text => format!("TEXT {}", Self::quote_value(value)?),
                    Conditions::To => format!("TO {}", Self::quote_value(value)?),
                    Conditions::Uid => {
                        let uid_value = value.ok_or_else(|| {
                            raise_error!(
                                "UID value is required".into(),
                                ErrorCode::InvalidParameter
                            )
                        })?;
                        Self::validate_uid(uid_value)?;
                        format!("UID {}", uid_value)
                    }
                    Conditions::Unanswered => "UNANSWERED".into(),
                    Conditions::Undeleted => "UNDELETED".into(),
                    Conditions::Undraft => "UNDRAFT".into(),
                    Conditions::Unflagged => "UNFLAGGED".into(),
                    Conditions::Unkeyword => format!("UNKEYWORD {}", Self::quote_value(value)?),
                    Conditions::Unseen => "UNSEEN".into(),
                };
                Ok(command)
            }
            Self::Logic(logic) => {
                let operator = match logic.operator {
                    Operator::And => " AND ",
                    Operator::Or => " OR ",
                    Operator::Not => " NOT ",
                };

                let children_commands: Vec<String> = logic
                    .children
                    .iter()
                    .map(|child| -> RustMailerResult<String> {
                        let child_command = child.to_imap_command()?;
                        if matches!(child, Self::Logic(_)) {
                            Ok(format!("({})", child_command))
                        } else {
                            Ok(child_command)
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                let command = children_commands.join(operator);
                if logic.operator == Operator::Not {
                    if command.starts_with('(') && command.ends_with(')') {
                        Ok(format!("NOT {}", command))
                    } else {
                        Ok(format!("NOT ({})", command))
                    }
                } else {
                    Ok(command)
                }
            }
        }
    }

    fn format_date(date: Option<&str>) -> RustMailerResult<String> {
        let date = date.ok_or_else(|| {
            raise_error!("Date value is required".into(), ErrorCode::InvalidParameter)
        })?;
        let naive_date = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| {
            raise_error!(
                "Invalid date format (expected YYYY-MM-DD)".into(),
                ErrorCode::InvalidParameter
            )
        })?;
        Ok(naive_date.format("%d-%b-%Y").to_string())
    }

    fn validate_number(number: Option<&str>) -> RustMailerResult<String> {
        let number = number.ok_or_else(|| {
            raise_error!(
                "Number value is required".into(),
                ErrorCode::InvalidParameter
            )
        })?;
        if number.parse::<u64>().is_err() {
            return Err(raise_error!(
                "Invalid number format".into(),
                ErrorCode::InvalidParameter
            ));
        }
        Ok(number.to_string())
    }

    fn validate_uid(uid_value: &str) -> RustMailerResult<()> {
        let uid_regex =
            regex::Regex::new(r"^(\d+|\d+:\d+)(\s+\d+|\s+\d+:\d+)*$").map_err(|_| {
                raise_error!(
                    "Failed to compile UID regex".into(),
                    ErrorCode::InvalidParameter
                )
            })?;

        if !uid_regex.is_match(uid_value) {
            return Err(raise_error!(
                "Invalid UID format (expected single number, multiple numbers, or range)".into(),
                ErrorCode::InvalidParameter
            ));
        }
        for part in uid_value.split_whitespace() {
            if part.contains(':') {
                let range_parts: Vec<&str> = part.split(':').collect();
                if range_parts.len() != 2 {
                    return Err(raise_error!(
                        "Invalid UID range format (expected 'start:end')".into(),
                        ErrorCode::InvalidParameter
                    ));
                }
                let start = range_parts[0].parse::<u64>().map_err(|_| {
                    raise_error!(
                        "Invalid UID range start value".into(),
                        ErrorCode::InvalidParameter
                    )
                })?;
                let end = range_parts[1].parse::<u64>().map_err(|_| {
                    raise_error!(
                        "Invalid UID range end value".into(),
                        ErrorCode::InvalidParameter
                    )
                })?;
                if start > end {
                    return Err(raise_error!(
                        "UID range start must be less than or equal to end".into(),
                        ErrorCode::InvalidParameter
                    ));
                }
            } else {
                part.parse::<u64>().map_err(|_| {
                    raise_error!("Invalid UID value".into(), ErrorCode::InvalidParameter)
                })?;
            }
        }

        Ok(())
    }

    fn quote_value(value: Option<&str>) -> RustMailerResult<String> {
        let value = value
            .ok_or_else(|| raise_error!("Value is required".into(), ErrorCode::InvalidParameter))?
            .trim();

        let value = if (value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\''))
        {
            &value[1..value.len() - 1]
        } else {
            value
        };

        let value = value.replace("\\\"", "\"").replace("\\'", "'");
        Ok(format!("\"{}\"", value))
    }
}


/// Request for searching messages in a specific mailbox  
///  
/// This structure combines search criteria with a target mailbox.  
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Object)]
pub struct MessageSearchRequest {
    /// The search criteria to apply (can be a simple condition or complex logical expression)  
    pub search: MessageSearch,
    /// The name of the mailbox to search in  
    pub mailbox: String,
}

impl MessageSearchRequest {
    fn cache_key(
        &self,
        account_id: u64,
        page: u64,
        page_size: u64,
        desc: bool,
        search_query: &str,
    ) -> String {
        format!(
            "{}_{}_{}_{}_{}_{}",
            account_id, self.mailbox, page, page_size, desc, search_query
        )
    }
    pub async fn search(
        &self,
        account_id: u64,
        page: u64,
        page_size: u64,
        desc: bool,
    ) -> RustMailerResult<DataPage<EmailEnvelope>> {
        let account = Account::check_account_active(account_id).await?;
        self.search_remote(&account, page, page_size, desc).await
    }


    async fn search_remote(
        &self,
        account: &Account,
        page: u64,
        page_size: u64,
        desc: bool,
    ) -> RustMailerResult<DataPage<EmailEnvelope>> {
        // Validate page and page_size
        if page == 0 || page_size == 0 {
            return Err(raise_error!(
                "Both page and page_size must be greater than 0.".into(),
                ErrorCode::InvalidParameter
            ));
        }
        if page_size > 1000 {
            return Err(raise_error!(
                "The page_size exceeds the maximum allowed limit of 1000.".into(),
                ErrorCode::InvalidParameter
            ));
        }

        let search_query = self.search.to_imap_command()?;
        info!(
            "Executing remote search for account_id: {}, mailbox: {}, with query: {}",
            account.id, self.mailbox, &search_query
        );
        let excutor = RUST_MAIL_CONTEXT.imap(account.id).await?;
        let cache_key = self.cache_key(account.id, page, page_size, desc, &search_query);

        // Attempt to retrieve from cache
        if let Some((uid_pages, total)) = IMAP_SEARCH_CACHE.get(&cache_key).await {
            let total_pages = (total as f64 / page_size as f64).ceil() as u64;

            if page > total_pages {
                return Ok(DataPage::new(
                    Some(page),
                    Some(page_size),
                    total,
                    Some(total_pages),
                    Vec::new(),
                ));
            }

            let current_page_uids = &uid_pages[(page - 1) as usize];
            let fetches = excutor
                .uid_fetch_meta(
                    current_page_uids,
                    encode_mailbox_name!(&self.mailbox).as_str(),
                    false,
                )
                .await?;
            let mut envelopes = Vec::new();
            for fetch in fetches {
                let envelope = extract_envelope(&fetch, account.id, &self.mailbox)?;
                envelopes.push(envelope);
            }

            return Ok(DataPage::new(
                Some(page),
                Some(page_size),
                total,
                Some(total_pages),
                envelopes,
            ));
        }

        // Cache miss, perform search and fetch data
        let uid_sets = excutor
            .uid_search(&encode_mailbox_name!(self.mailbox), &search_query)
            .await?;
        if uid_sets.is_empty() {
            IMAP_SEARCH_CACHE.set(cache_key, Arc::new(vec![]), 0).await;
            return Ok(DataPage::new(
                Some(page),
                Some(page_size),
                0,
                None,
                Vec::new(),
            ));
        }

        let total_items = uid_sets.len() as u64;
        let total_pages = (total_items as f64 / page_size as f64).ceil() as u64;
        let pages = Arc::new(generate_uid_sequence_hashset(
            uid_sets,
            page_size as usize,
            desc,
        ));
        assert_eq!(total_pages, pages.len() as u64);
        IMAP_SEARCH_CACHE
            .set(cache_key, pages.clone(), total_items)
            .await;

        if page > total_pages {
            return Ok(DataPage::new(
                Some(page),
                Some(page_size),
                total_items,
                Some(total_pages),
                Vec::new(),
            ));
        }

        let current_page_uids = &pages[(page - 1) as usize];
        let fetches = excutor
            .uid_fetch_meta(
                current_page_uids,
                &encode_mailbox_name!(self.mailbox),
                false,
            )
            .await?;
        let mut envelopes = Vec::new();
        for fetch in fetches {
            let envelope = extract_envelope(&fetch, account.id, &self.mailbox)?;
            envelopes.push(envelope);
        }

        Ok(DataPage::new(
            Some(page),
            Some(page_size),
            total_items,
            Some(total_pages),
            envelopes,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn test_generate_uid_sequence_hashset_odd_chunk_size() {
        let unique_nums: HashSet<u32> = [1, 2, 3, 4, 5, 6, 7, 10].iter().cloned().collect();
        let chunk_size = 3;

        let result = generate_uid_sequence_hashset(unique_nums.clone(), chunk_size, false);
        assert_eq!(result, vec!["1:3", "4:6", "7,10"]);
        let result = generate_uid_sequence_hashset(unique_nums.clone(), chunk_size, true);
        assert_eq!(result, vec!["6:7,10", "3:5", "1:2"]);
    }

    #[test]
    fn test_condition_node_all() {
        let condition_node = MessageSearch::Condition(Condition {
            condition: Conditions::All,
            value: None,
        });

        let result = condition_node.to_imap_command();
        assert_eq!(result.unwrap(), "ALL");
    }

    #[test]
    fn test_condition_node_from() {
        let condition_node = MessageSearch::Condition(Condition {
            condition: Conditions::From,
            value: Some("example@example.com".to_string()),
        });

        let result = condition_node.to_imap_command();
        assert_eq!(result.unwrap(), "FROM \"example@example.com\"");
    }

    #[test]
    fn test_condition_node_subject() {
        let condition_node = MessageSearch::Condition(Condition {
            condition: Conditions::Subject,
            value: Some("Hello".to_string()),
        });

        let result = condition_node.to_imap_command();
        assert_eq!(result.unwrap(), "SUBJECT \"Hello\"");
    }

    #[test]
    fn test_condition_node_date() {
        let condition_node = MessageSearch::Condition(Condition {
            condition: Conditions::On,
            value: Some("2025-02-03".to_string()),
        });

        let result = condition_node.to_imap_command();
        assert_eq!(result.unwrap(), "ON 03-Feb-2025");
    }

    #[test]
    fn test_logic_node_and() {
        let condition1 = MessageSearch::Condition(Condition {
            condition: Conditions::From,
            value: Some("example@example.com".to_string()),
        });

        let condition2 = MessageSearch::Condition(Condition {
            condition: Conditions::Subject,
            value: Some("Hello".to_string()),
        });

        let logic_node = MessageSearch::Logic(Logic {
            operator: Operator::And,
            children: vec![condition1, condition2],
        });

        let result = logic_node.to_imap_command();
        assert_eq!(
            result.unwrap(),
            "FROM \"example@example.com\" AND SUBJECT \"Hello\""
        );
    }

    #[test]
    fn test_logic_node_or() {
        let condition1 = MessageSearch::Condition(Condition {
            condition: Conditions::From,
            value: Some("example@example.com".to_string()),
        });

        let condition2 = MessageSearch::Condition(Condition {
            condition: Conditions::Subject,
            value: Some("Hello".to_string()),
        });

        let logic_node = MessageSearch::Logic(Logic {
            operator: Operator::Or,
            children: vec![condition1, condition2],
        });

        let result = logic_node.to_imap_command();
        assert_eq!(
            result.unwrap(),
            "FROM \"example@example.com\" OR SUBJECT \"Hello\""
        );
    }

    #[test]
    fn test_logic_node_not() {
        let condition = MessageSearch::Condition(Condition {
            condition: Conditions::Subject,
            value: Some("Hello".to_string()),
        });

        let logic_node = MessageSearch::Logic(Logic {
            operator: Operator::Not,
            children: vec![condition],
        });

        let result = logic_node.to_imap_command();
        assert_eq!(result.unwrap(), "NOT (SUBJECT \"Hello\")");
    }

    #[test]
    fn test_nested_logic_nodes() {
        let condition1 = MessageSearch::Condition(Condition {
            condition: Conditions::From,
            value: Some("example@example.com".to_string()),
        });

        let condition2 = MessageSearch::Condition(Condition {
            condition: Conditions::Subject,
            value: Some("Hello".to_string()),
        });

        let logic_node1 = MessageSearch::Logic(Logic {
            operator: Operator::And,
            children: vec![condition1, condition2],
        });

        let condition3 = MessageSearch::Condition(Condition {
            condition: Conditions::To,
            value: Some("receiver@example.com".to_string()),
        });

        let logic_node2 = MessageSearch::Logic(Logic {
            operator: Operator::Or,
            children: vec![logic_node1, condition3],
        });

        let result = logic_node2.to_imap_command();
        assert_eq!(
            result.unwrap(),
            "(FROM \"example@example.com\" AND SUBJECT \"Hello\") OR TO \"receiver@example.com\""
        );
    }

    #[test]
    fn test_not() {
        let condition1 = MessageSearch::Condition(Condition {
            condition: Conditions::From,
            value: Some("example@example.com".to_string()),
        });

        let condition2 = MessageSearch::Condition(Condition {
            condition: Conditions::Subject,
            value: Some("Hello".to_string()),
        });

        let logic_node1 = MessageSearch::Logic(Logic {
            operator: Operator::And,
            children: vec![condition1, condition2],
        });

        let logic_node2 = MessageSearch::Logic(Logic {
            operator: Operator::Not,
            children: vec![logic_node1],
        });

        let result = logic_node2.to_imap_command();
        println!("{}", result.unwrap());
    }

    #[test]
    fn test_invalid_header_format() {
        let condition_node = MessageSearch::Condition(Condition {
            condition: Conditions::Header,
            value: Some("InvalidHeader".to_string()), // Invalid header format
        });

        let result = condition_node.to_imap_command();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_date_format() {
        let condition_node = MessageSearch::Condition(Condition {
            condition: Conditions::On,
            value: Some("invalid-date".to_string()), // Invalid date format
        });

        let result = condition_node.to_imap_command();
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_number_format() {
        let condition_node = MessageSearch::Condition(Condition {
            condition: Conditions::Larger,
            value: Some("not-a-number".to_string()), // Invalid number format
        });

        let result = condition_node.to_imap_command();
        assert!(result.is_err());
    }
}
