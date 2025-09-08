// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use std::sync::LazyLock;
use std::time::Duration;

use ahash::AHashMap;

use crate::modules::common::lru::TimedLruCache;

pub static GMAIL_LABELS_CACHE: LazyLock<TimedLruCache<u64, AHashMap<String, String>>> =
    LazyLock::new(|| TimedLruCache::new(100, Duration::from_secs(3600)));
