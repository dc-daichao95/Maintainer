use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct RemoteStats {
    pub status: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub pending: Option<i64>,
    #[serde(default)]
    pub reviewing: Option<i64>,
    #[serde(default)]
    pub messages: Option<i64>,
    #[serde(default)]
    pub patchsets: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RemoteFindings {
    pub total: u32,
    #[serde(default)]
    pub pending: u32,
    pub not_issue: u32,
    pub to_fix: u32,
    pub fixed: u32,
}

#[derive(Debug, Deserialize)]
pub struct TimelinePoint {
    pub day: String,
    pub count: u32,
}

#[derive(Debug, Deserialize)]
pub struct TimelineStatusPoint {
    pub day: String,
    pub status: String,
    pub count: u32,
}

#[derive(Debug, Deserialize)]
pub struct RemoteTimeline {
    #[serde(default)]
    pub messages: Vec<TimelinePoint>,
    #[serde(default)]
    pub patchsets: Vec<TimelineStatusPoint>,
    #[serde(default)]
    pub patches: Vec<TimelinePoint>,
    #[serde(default)]
    pub reviews: Vec<TimelineStatusPoint>,
}

/// Trait for fetching remote stats.
#[async_trait::async_trait]
pub trait RemoteStatsFetcher: Send + Sync {
    async fn fetch_stats(&self, base_url: &str) -> Result<RemoteStats>;
    async fn fetch_findings(&self, base_url: &str) -> Result<RemoteFindings>;
    async fn fetch_timeline(&self, base_url: &str) -> Result<RemoteTimeline>;
}

/// HTTP implementation of RemoteStatsFetcher.
#[derive(Clone)]
pub struct HttpStatsFetcher {
    client: reqwest::Client,
}

impl HttpStatsFetcher {
    pub fn new() -> Self {
        // Remote sashiko nodes live on the internal LAN. A system/corporate
        // HTTP proxy (picked up from env by default) cannot reach those hosts
        // and would make every node appear offline, so we bypass any proxy.
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .no_proxy()
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait::async_trait]
impl RemoteStatsFetcher for HttpStatsFetcher {
    async fn fetch_stats(&self, base_url: &str) -> Result<RemoteStats> {
        let url = format!("{}/api/stats", base_url);
        let res = self.client.get(&url).send().await?.json::<RemoteStats>().await?;
        Ok(res)
    }

    async fn fetch_findings(&self, base_url: &str) -> Result<RemoteFindings> {
        let url = format!("{}/api/stats/findings", base_url);
        let res = self.client.get(&url).send().await?.json::<RemoteFindings>().await?;
        Ok(res)
    }

    async fn fetch_timeline(&self, base_url: &str) -> Result<RemoteTimeline> {
        let url = format!("{}/api/stats/timeline", base_url);
        let res = self.client.get(&url).send().await?.json::<RemoteTimeline>().await?;
        Ok(res)
    }
}

use tokio::sync::RwLock;
use std::time::Instant;

/// Sector data for a pie chart.
#[derive(Debug, Serialize, Clone)]
pub struct PieSector {
    pub name: String,
    pub value: u32,
}

/// One server's trend line, with counts aligned to [`DashboardStats::trend_days`].
///
/// Keeping counts aligned to a shared day axis lets the frontend stack multiple
/// servers into a single stacked-area chart without re-aligning on the client.
#[derive(Debug, Serialize, Clone)]
pub struct ServerTrendSeries {
    pub name: String,
    pub counts: Vec<u32>,
}

/// Server card data for the dashboard.
#[derive(Debug, Serialize, Clone)]
pub struct ServerCard {
    pub id: i64,
    pub name: String,
    pub ip: String,
    pub web_port: u16,
    pub description: String,
    pub online: bool,
}

/// Aggregated dashboard stats.
#[derive(Debug, Serialize, Clone)]
pub struct DashboardStats {
    pub total_patchsets: i64,
    pub total_issues: u32,
    pub avg_accuracy: f64,
    pub online_servers: u32,
    pub offline_servers: u32,
    pub pie_chart_data: Vec<PieSector>,
    /// Shared x-axis days for the trend chart, sorted ascending.
    pub trend_days: Vec<String>,
    /// One trend series per configured server, each aligned to `trend_days`.
    pub trend_series: Vec<ServerTrendSeries>,
    pub servers: Vec<ServerCard>,
}

/// The aggregation service that fetches stats from remote servers and aggregates them.
pub struct AggregationService<F: RemoteStatsFetcher> {
    fetcher: F,
    cache: RwLock<Option<(Instant, DashboardStats)>>,
}

impl<F: RemoteStatsFetcher> AggregationService<F> {
    pub fn new(fetcher: F) -> Self {
        Self { 
            fetcher,
            cache: RwLock::new(None),
        }
    }

    pub async fn aggregate(&self, servers: Vec<crate::models::ServerConfig>) -> DashboardStats {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some((timestamp, stats)) = &*cache {
                if timestamp.elapsed() < Duration::from_secs(30) {
                    return stats.clone();
                }
            }
        }

        let results = self.fetch_all(&servers).await;
        let stats = self.process_results(results);

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some((Instant::now(), stats.clone()));
        }

        stats
    }

    async fn fetch_all<'a>(&'a self, servers: &'a [crate::models::ServerConfig]) -> Vec<(&'a crate::models::ServerConfig, bool, Result<RemoteStats>, Result<RemoteFindings>, Result<RemoteTimeline>)> {
        let futures = servers.iter().map(|server| {
            let base_url = format!("http://{}:{}", server.ip, server.web_port);
            async move {
                let stats_fut = self.fetcher.fetch_stats(&base_url);
                let findings_fut = self.fetcher.fetch_findings(&base_url);
                let timeline_fut = self.fetcher.fetch_timeline(&base_url);

                let (stats_res, findings_res, timeline_res) = tokio::join!(stats_fut, findings_fut, timeline_fut);

                let online = match &stats_res {
                    Ok(s) => s.status == "ok",
                    Err(e) => {
                        // Surface the real reason (timeout/connection refused/...)
                        // so an offline node can actually be diagnosed.
                        tracing::warn!(
                            server = %server.name,
                            url = %base_url,
                            error = %e,
                            "remote sashiko status probe failed; marking offline"
                        );
                        false
                    }
                };

                (server, online, stats_res, findings_res, timeline_res)
            }
        });

        futures::future::join_all(futures).await
    }

    fn process_results(&self, results: Vec<(&crate::models::ServerConfig, bool, Result<RemoteStats>, Result<RemoteFindings>, Result<RemoteTimeline>)>) -> DashboardStats {
        let mut total_patchsets = 0;
        let mut total_issues = 0;
        let mut not_issue_total = 0;
        let mut to_fix_total = 0;
        let mut fixed_total = 0;
        let mut online_servers = 0;
        let mut offline_servers = 0;
        let mut pie_chart_data = vec![];
        let mut server_cards = vec![];

        // Per-server day->count maps; the outer Vec preserves the configured
        // server order so stacked series line up with the cards and pie chart.
        let mut per_server_days: Vec<(String, std::collections::HashMap<String, u32>)> = vec![];
        // BTreeSet keeps the shared day axis sorted ascending automatically.
        let mut all_days: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        for (server, online, stats, findings, timeline) in results {
            if online {
                online_servers += 1;
            } else {
                offline_servers += 1;
            }

            if let Ok(s) = stats {
                total_patchsets += s.patchsets.unwrap_or(0);
            }

            let mut server_issues = 0;

            if let Ok(f) = findings {
                server_issues = f.total;
                total_issues += f.total;
                not_issue_total += f.not_issue;
                to_fix_total += f.to_fix;
                fixed_total += f.fixed;
            }

            let mut day_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
            if let Ok(t) = timeline {
                for pt in t.messages {
                    all_days.insert(pt.day.clone());
                    *day_counts.entry(pt.day).or_insert(0) += pt.count;
                }
            }
            per_server_days.push((server.name.clone(), day_counts));

            pie_chart_data.push(PieSector {
                name: server.name.clone(),
                value: server_issues,
            });

            server_cards.push(ServerCard {
                id: server.id.unwrap_or(0),
                name: server.name.clone(),
                ip: server.ip.clone(),
                web_port: server.web_port,
                description: server.description.clone(),
                online,
            });
        }

        let denominator = not_issue_total + to_fix_total + fixed_total;
        let avg_accuracy = if denominator == 0 {
            0.0
        } else {
            (to_fix_total + fixed_total) as f64 / denominator as f64
        };

        let trend_days: Vec<String> = all_days.into_iter().collect();
        // Align every server's counts to the shared day axis, filling gaps with
        // zero so offline or sparse servers still stack cleanly on the chart.
        let trend_series: Vec<ServerTrendSeries> = per_server_days
            .into_iter()
            .map(|(name, day_counts)| {
                let counts = trend_days
                    .iter()
                    .map(|day| *day_counts.get(day).unwrap_or(&0))
                    .collect();
                ServerTrendSeries { name, counts }
            })
            .collect();

        DashboardStats {
            total_patchsets,
            total_issues,
            avg_accuracy,
            online_servers,
            offline_servers,
            pie_chart_data,
            trend_days,
            trend_series,
            servers: server_cards,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockFetcher {
        online: bool,
    }

    #[async_trait::async_trait]
    impl RemoteStatsFetcher for MockFetcher {
        async fn fetch_stats(&self, _base_url: &str) -> Result<RemoteStats> {
            if self.online {
                Ok(RemoteStats {
                    status: "ok".into(),
                    version: None,
                    pending: None,
                    reviewing: None,
                    messages: None,
                    patchsets: Some(42),
                })
            } else {
                anyhow::bail!("offline")
            }
        }

        async fn fetch_findings(&self, _base_url: &str) -> Result<RemoteFindings> {
            if self.online {
                Ok(RemoteFindings {
                    total: 100,
                    pending: 10,
                    not_issue: 20,
                    to_fix: 30,
                    fixed: 40,
                })
            } else {
                anyhow::bail!("offline")
            }
        }

        async fn fetch_timeline(&self, _base_url: &str) -> Result<RemoteTimeline> {
            if self.online {
                Ok(RemoteTimeline {
                    messages: vec![TimelinePoint { day: "2023-01-01".into(), count: 5 }],
                    patchsets: vec![],
                    patches: vec![],
                    reviews: vec![],
                })
            } else {
                anyhow::bail!("offline")
            }
        }
    }

    #[tokio::test]
    async fn test_aggregation() {
        let fetcher = MockFetcher { online: true };
        let service = AggregationService::new(fetcher);

        let servers = vec![
            crate::models::ServerConfig {
                id: Some(1),
                name: "Server 1".into(),
                ip: "127.0.0.1".into(),
                web_port: 8080,
                description: "".into(),
            },
        ];

        let stats = service.aggregate(servers).await;
        assert_eq!(stats.online_servers, 1);
        assert_eq!(stats.offline_servers, 0);
        assert_eq!(stats.total_issues, 100);
        assert_eq!(stats.total_patchsets, 42);
        
        // (30 + 40) / (20 + 30 + 40) = 70 / 90 = 0.777...
        assert!((stats.avg_accuracy - 0.7777).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_aggregation_zero_servers() {
        let fetcher = MockFetcher { online: true };
        let service = AggregationService::new(fetcher);

        let stats = service.aggregate(vec![]).await;
        assert_eq!(stats.online_servers, 0);
        assert_eq!(stats.offline_servers, 0);
        assert_eq!(stats.total_issues, 0);
        assert_eq!(stats.avg_accuracy, 0.0);
    }

    #[tokio::test]
    async fn test_aggregation_fault_tolerance() {
        struct FaultyFetcher;
        #[async_trait::async_trait]
        impl RemoteStatsFetcher for FaultyFetcher {
            async fn fetch_stats(&self, base_url: &str) -> Result<RemoteStats> {
                if base_url.contains("8080") {
                    Ok(RemoteStats {
                        status: "ok".into(),
                        version: None,
                        pending: None,
                        reviewing: None,
                        messages: None,
                        patchsets: Some(15),
                    })
                } else {
                    anyhow::bail!("timeout")
                }
            }
            async fn fetch_findings(&self, base_url: &str) -> Result<RemoteFindings> {
                if base_url.contains("8080") {
                    Ok(RemoteFindings {
                        total: 50,
                        pending: 0,
                        not_issue: 10,
                        to_fix: 20,
                        fixed: 20,
                    })
                } else {
                    anyhow::bail!("timeout")
                }
            }
            async fn fetch_timeline(&self, base_url: &str) -> Result<RemoteTimeline> {
                if base_url.contains("8080") {
                    Ok(RemoteTimeline {
                        messages: vec![],
                        patchsets: vec![],
                        patches: vec![],
                        reviews: vec![],
                    })
                } else {
                    anyhow::bail!("timeout")
                }
            }
        }

        let service = AggregationService::new(FaultyFetcher);
        let servers = vec![
            crate::models::ServerConfig {
                id: Some(1),
                name: "Server 1".into(),
                ip: "127.0.0.1".into(),
                web_port: 8080,
                description: "".into(),
            },
            crate::models::ServerConfig {
                id: Some(2),
                name: "Server 2".into(),
                ip: "127.0.0.1".into(),
                web_port: 8081,
                description: "".into(),
            },
        ];

        let stats = service.aggregate(servers).await;
        assert_eq!(stats.online_servers, 1);
        assert_eq!(stats.offline_servers, 1);
        assert_eq!(stats.total_issues, 50);
        // Only the reachable node (8080) contributes its patchset count.
        assert_eq!(stats.total_patchsets, 15);
        // (20 + 20) / (10 + 20 + 20) = 40 / 50 = 0.8
        assert!((stats.avg_accuracy - 0.8).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_aggregation_divide_by_zero_guard() {
        struct ZeroFetcher;
        #[async_trait::async_trait]
        impl RemoteStatsFetcher for ZeroFetcher {
            async fn fetch_stats(&self, _base_url: &str) -> Result<RemoteStats> {
                Ok(RemoteStats {
                    status: "ok".into(),
                    version: None,
                    pending: None,
                    reviewing: None,
                    messages: None,
                    patchsets: None,
                })
            }
            async fn fetch_findings(&self, _base_url: &str) -> Result<RemoteFindings> {
                Ok(RemoteFindings {
                    total: 0,
                    pending: 0,
                    not_issue: 0,
                    to_fix: 0,
                    fixed: 0,
                })
            }
            async fn fetch_timeline(&self, _base_url: &str) -> Result<RemoteTimeline> {
                Ok(RemoteTimeline {
                    messages: vec![],
                    patchsets: vec![],
                    patches: vec![],
                    reviews: vec![],
                })
            }
        }

        let service = AggregationService::new(ZeroFetcher);
        let servers = vec![crate::models::ServerConfig {
            id: Some(1),
            name: "Server 1".into(),
            ip: "127.0.0.1".into(),
            web_port: 8080,
            description: "".into(),
        }];

        let stats = service.aggregate(servers).await;
        assert_eq!(stats.online_servers, 1);
        assert_eq!(stats.total_issues, 0);
        assert_eq!(stats.avg_accuracy, 0.0);
    }

    #[tokio::test]
    async fn test_aggregation_multiple_servers_math() {
        struct MultiFetcher;
        #[async_trait::async_trait]
        impl RemoteStatsFetcher for MultiFetcher {
            async fn fetch_stats(&self, _base_url: &str) -> Result<RemoteStats> {
                Ok(RemoteStats {
                    status: "ok".into(),
                    version: None,
                    pending: None,
                    reviewing: None,
                    messages: None,
                    patchsets: None,
                })
            }
            async fn fetch_findings(&self, base_url: &str) -> Result<RemoteFindings> {
                if base_url.contains("8080") {
                    Ok(RemoteFindings {
                        total: 10,
                        pending: 0,
                        not_issue: 2,
                        to_fix: 3,
                        fixed: 5,
                    })
                } else {
                    Ok(RemoteFindings {
                        total: 20,
                        pending: 0,
                        not_issue: 8,
                        to_fix: 7,
                        fixed: 5,
                    })
                }
            }
            async fn fetch_timeline(&self, _base_url: &str) -> Result<RemoteTimeline> {
                Ok(RemoteTimeline {
                    messages: vec![],
                    patchsets: vec![],
                    patches: vec![],
                    reviews: vec![],
                })
            }
        }

        let service = AggregationService::new(MultiFetcher);
        let servers = vec![
            crate::models::ServerConfig {
                id: Some(1),
                name: "Server 1".into(),
                ip: "127.0.0.1".into(),
                web_port: 8080,
                description: "".into(),
            },
            crate::models::ServerConfig {
                id: Some(2),
                name: "Server 2".into(),
                ip: "127.0.0.1".into(),
                web_port: 8081,
                description: "".into(),
            },
        ];

        let stats = service.aggregate(servers).await;
        assert_eq!(stats.online_servers, 2);
        assert_eq!(stats.total_issues, 30);
        // Server 1: to_fix=3, fixed=5, not_issue=2
        // Server 2: to_fix=7, fixed=5, not_issue=8
        // Total: to_fix=10, fixed=10, not_issue=10
        // Accuracy = (10 + 10) / (10 + 10 + 10) = 20 / 30 = 0.666...
        assert!((stats.avg_accuracy - 0.6666).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_aggregation_per_server_trend_alignment() {
        // Two servers report timelines on partially overlapping days. The
        // aggregate must expose one series per server, each aligned to the
        // unified, sorted day axis with zero-fill for missing days.
        struct TrendFetcher;
        #[async_trait::async_trait]
        impl RemoteStatsFetcher for TrendFetcher {
            async fn fetch_stats(&self, _base_url: &str) -> Result<RemoteStats> {
                Ok(RemoteStats {
                    status: "ok".into(),
                    version: None,
                    pending: None,
                    reviewing: None,
                    messages: None,
                    patchsets: None,
                })
            }
            async fn fetch_findings(&self, _base_url: &str) -> Result<RemoteFindings> {
                Ok(RemoteFindings {
                    total: 0,
                    pending: 0,
                    not_issue: 0,
                    to_fix: 0,
                    fixed: 0,
                })
            }
            async fn fetch_timeline(&self, base_url: &str) -> Result<RemoteTimeline> {
                if base_url.contains("8080") {
                    Ok(RemoteTimeline {
                        messages: vec![
                            TimelinePoint { day: "2023-01-01".into(), count: 5 },
                            TimelinePoint { day: "2023-01-02".into(), count: 7 },
                        ],
                        patchsets: vec![],
                        patches: vec![],
                        reviews: vec![],
                    })
                } else {
                    Ok(RemoteTimeline {
                        messages: vec![
                            TimelinePoint { day: "2023-01-02".into(), count: 3 },
                            TimelinePoint { day: "2023-01-03".into(), count: 9 },
                        ],
                        patchsets: vec![],
                        patches: vec![],
                        reviews: vec![],
                    })
                }
            }
        }

        let service = AggregationService::new(TrendFetcher);
        let servers = vec![
            crate::models::ServerConfig {
                id: Some(1),
                name: "Node-A".into(),
                ip: "127.0.0.1".into(),
                web_port: 8080,
                description: "".into(),
            },
            crate::models::ServerConfig {
                id: Some(2),
                name: "Node-B".into(),
                ip: "127.0.0.1".into(),
                web_port: 8081,
                description: "".into(),
            },
        ];

        let stats = service.aggregate(servers).await;

        // Unified, sorted day axis spanning both servers.
        assert_eq!(stats.trend_days, vec!["2023-01-01", "2023-01-02", "2023-01-03"]);
        // One series per configured server, in configured order.
        assert_eq!(stats.trend_series.len(), 2);
        assert_eq!(stats.trend_series[0].name, "Node-A");
        assert_eq!(stats.trend_series[1].name, "Node-B");
        // Missing days zero-filled so the stacked areas align.
        assert_eq!(stats.trend_series[0].counts, vec![5, 7, 0]);
        assert_eq!(stats.trend_series[1].counts, vec![0, 3, 9]);
    }

    #[tokio::test]
    async fn test_aggregation_cache() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        struct CountingFetcher {
            call_count: Arc<AtomicUsize>,
        }

        #[async_trait::async_trait]
        impl RemoteStatsFetcher for CountingFetcher {
            async fn fetch_stats(&self, _base_url: &str) -> Result<RemoteStats> {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                Ok(RemoteStats {
                    status: "ok".into(),
                    version: None,
                    pending: None,
                    reviewing: None,
                    messages: None,
                    patchsets: None,
                })
            }
            async fn fetch_findings(&self, _base_url: &str) -> Result<RemoteFindings> {
                Ok(RemoteFindings {
                    total: 0,
                    pending: 0,
                    not_issue: 0,
                    to_fix: 0,
                    fixed: 0,
                })
            }
            async fn fetch_timeline(&self, _base_url: &str) -> Result<RemoteTimeline> {
                Ok(RemoteTimeline {
                    messages: vec![],
                    patchsets: vec![],
                    patches: vec![],
                    reviews: vec![],
                })
            }
        }

        let call_count = Arc::new(AtomicUsize::new(0));
        let fetcher = CountingFetcher { call_count: call_count.clone() };
        let service = AggregationService::new(fetcher);

        let servers = vec![crate::models::ServerConfig {
            id: Some(1),
            name: "Server 1".into(),
            ip: "127.0.0.1".into(),
            web_port: 8080,
            description: "".into(),
        }];

        // First call
        let _ = service.aggregate(servers.clone()).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Second call (should hit cache)
        let _ = service.aggregate(servers).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
