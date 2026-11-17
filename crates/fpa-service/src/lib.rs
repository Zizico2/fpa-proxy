//! Generated HTTP clients are the sole source boundary; conversion preserves missing values.
mod convert;
use convert::*;
use fpa_proto::v1::{self as pb, results_service_server::ResultsService};
use prost::Message;
use serde_json::json;
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Semaphore;
use tonic::{Request, Response, Status};
pub struct ProxyOptions {
    pub upstream_base_url: reqwest::Url,
    pub portal_base_url: reqwest::Url,
    pub upstream_timeout: Duration,
    pub max_concurrent_fetches: usize,
    pub max_response_bytes: usize,
}
#[derive(Clone)]
pub struct Proxy {
    ws: fpa_ws_client::Client,
    portal: fpa_portal_client::Client,
    permits: Arc<Semaphore>,
    max_bytes: usize,
}
fn now() -> prost_types::Timestamp {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    prost_types::Timestamp {
        seconds: d.as_secs() as i64,
        nanos: d.subsec_nanos() as i32,
    }
}
fn id(v: &str) -> Result<&str, Status> {
    if v.is_empty()
        || v.len() > 12
        || !v.bytes().all(|c| c.is_ascii_digit())
        || v.parse::<u64>().unwrap_or(0) == 0
    {
        return Err(Status::invalid_argument(
            "positive source identifier required",
        ));
    }
    Ok(v)
}
fn upstream(e: impl std::fmt::Display) -> Status {
    tracing::warn!(error=%e,"FPA source failed");
    Status::unavailable("FPA source unavailable")
}
impl Proxy {
    pub fn new(o: ProxyOptions) -> anyhow::Result<Self> {
        anyhow::ensure!(
            o.max_concurrent_fetches > 0 && o.max_response_bytes > 0,
            "positive limits required"
        );
        for u in [&o.upstream_base_url, &o.portal_base_url] {
            anyhow::ensure!(
                u.username().is_empty()
                    && u.password().is_none()
                    && u.query().is_none()
                    && u.fragment().is_none(),
                "invalid source URL"
            );
            anyhow::ensure!(
                u.scheme() == "https"
                    || (u.scheme() == "http"
                        && matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "::1"))),
                "HTTP requires loopback"
            );
        }
        let http = reqwest::Client::builder()
            .user_agent("fpa-proxy/0.1")
            .timeout(o.upstream_timeout)
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .gzip(true)
            .build()?;
        Ok(Self {
            ws: fpa_ws_client::Client::new_with_client(
                o.upstream_base_url.as_str().trim_end_matches('/'),
                http.clone(),
            ),
            portal: fpa_portal_client::Client::new_with_client(
                o.portal_base_url.as_str().trim_end_matches('/'),
                http,
            ),
            permits: Arc::new(Semaphore::new(o.max_concurrent_fetches)),
            max_bytes: o.max_response_bytes,
        })
    }
    async fn permit(&self) -> Result<tokio::sync::OwnedSemaphorePermit, Status> {
        self.permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| Status::resource_exhausted("source capacity exhausted"))
    }
    fn response<T: Message>(&self, v: T) -> Result<Response<T>, Status> {
        if v.encoded_len() > self.max_bytes {
            return Err(Status::resource_exhausted("response too large"));
        }
        Ok(Response::new(v))
    }
}
#[tonic::async_trait]
impl ResultsService for Proxy {
    async fn ping(
        &self,
        _: Request<pb::PingRequest>,
    ) -> Result<Response<pb::PingResponse>, Status> {
        Ok(Response::new(pb::PingResponse {
            message: "fpa-proxy ready".into(),
        }))
    }
    async fn list_seasons(
        &self,
        _: Request<pb::ListSeasonsRequest>,
    ) -> Result<Response<pb::ListSeasonsResponse>, Status> {
        let _p = self.permit().await?;
        let mut seasons = vec![];
        let mut page = 1;
        loop {
            let r = self
                .portal
                .list_seasons()
                .per_page(100)
                .page(page)
                .send()
                .await
                .map_err(upstream)?;
            let pages = r
                .headers()
                .get("x-wp-totalpages")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(1);
            for row in r.into_inner() {
                if let Some(n) = row.slug.strip_prefix("season-").filter(|v| id(v).is_ok()) {
                    seasons.push(pb::Season {
                        id: n.into(),
                        name: row.name.replace("Época", "").trim().into(),
                    });
                }
            }
            if page >= pages {
                break;
            }
            page += 1;
            if page > 20 {
                return Err(Status::resource_exhausted("season pagination limit"));
            }
        }
        seasons.sort_by(|a, b| {
            b.id.parse::<u64>()
                .unwrap_or(0)
                .cmp(&a.id.parse::<u64>().unwrap_or(0))
        });
        seasons.dedup_by(|a, b| a.id == b.id);
        self.response(pb::ListSeasonsResponse {
            seasons,
            fetched_at: Some(now()),
        })
    }
    async fn list_competitions(
        &self,
        r: Request<pb::ListCompetitionsRequest>,
    ) -> Result<Response<pb::ListCompetitionsResponse>, Status> {
        let r = r.into_inner();
        id(&r.season_id)?;
        let a = if r.association_id.is_empty() {
            "8"
        } else {
            id(&r.association_id)?
        };
        if !matches!(r.gender.as_str(), "M" | "F") {
            return Err(Status::invalid_argument("gender must be M or F"));
        }
        let _p = self.permit().await?;
        let rows = self
            .ws
            .list_competitions()
            .season_id(r.season_id)
            .association_id(a)
            .gender(r.gender)
            .send()
            .await
            .map_err(upstream)?
            .into_inner();
        let competitions = rows
            .iter()
            .map(|r| json!(r))
            .map(|r| pb::Competition {
                id: text(&r, "ID_PROVA"),
                name: text(&r, "PRO_DESCRICAO"),
                code: text(&r, "NCO_CODIGO"),
            })
            .collect();
        self.response(pb::ListCompetitionsResponse {
            competitions,
            fetched_at: Some(now()),
        })
    }
    async fn list_phases(
        &self,
        r: Request<pb::ListPhasesRequest>,
    ) -> Result<Response<pb::ListPhasesResponse>, Status> {
        let r = r.into_inner();
        id(&r.competition_id)?;
        let _p = self.permit().await?;
        let rows = self
            .ws
            .list_phases()
            .competition_id(r.competition_id)
            .send()
            .await
            .map_err(upstream)?
            .into_inner();
        self.response(pb::ListPhasesResponse {
            phases: rows
                .iter()
                .map(|r| json!(r))
                .map(|r| pb::Phase {
                    id: text(&r, "ID_FASE"),
                    name: text(&r, "DESCRICAO"),
                })
                .collect(),
            fetched_at: Some(now()),
        })
    }
    async fn get_phase(
        &self,
        r: Request<pb::GetPhaseRequest>,
    ) -> Result<Response<pb::GetPhaseResponse>, Status> {
        let r = r.into_inner();
        id(&r.competition_id)?;
        id(&r.phase_id)?;
        let _p = self.permit().await?;
        let (games, table) = tokio::try_join!(
            self.ws
                .list_fixtures()
                .competition_id(&r.competition_id)
                .phase_id(&r.phase_id)
                .send(),
            self.ws
                .get_standings()
                .competition_id(&r.competition_id)
                .phase_id(&r.phase_id)
                .send()
        )
        .map_err(upstream)?;
        self.response(pb::GetPhaseResponse {
            matches: games
                .into_inner()
                .iter()
                .map(|r| fixture(&json!(r), false))
                .collect(),
            standings: table
                .into_inner()
                .iter()
                .map(|r| standing(&json!(r)))
                .collect(),
            fetched_at: Some(now()),
        })
    }
    async fn get_match(
        &self,
        r: Request<pb::GetMatchRequest>,
    ) -> Result<Response<pb::GetMatchResponse>, Status> {
        let r = r.into_inner();
        id(&r.match_id)?;
        let _p = self.permit().await?;
        let v = json!(
            self.ws
                .get_match()
                .match_id(r.match_id)
                .send()
                .await
                .map_err(upstream)?
                .into_inner()
        );
        let g = v["jogoGeral"]
            .as_array()
            .and_then(|a| a.first())
            .ok_or_else(|| Status::not_found("match not found"))?;
        let periods = [
            ("1.ª parte", "1P"),
            ("2.ª parte", "2P"),
            ("Prolongamento 1", "TE"),
            ("Prolongamento 2", "TE2"),
            ("7 metros", "7M"),
        ]
        .iter()
        .filter_map(|(label, k)| {
            let home = uint(g, &format!("JOG_GOLOS_{k}_CASA"));
            let away = uint(g, &format!("JOG_GOLOS_{k}_FORA"));
            (home.is_some() || away.is_some()).then(|| pb::Period {
                name: (*label).into(),
                home,
                away,
            })
        })
        .collect();
        let people = |key: &str| {
            v[key]
                .as_array()
                .map(|rows| rows.iter().map(participant).collect())
                .unwrap_or_default()
        };
        self.response(pb::GetMatchResponse {
            r#match: Some(fixture(g, true)),
            periods,
            home_players: people("equipaCasa"),
            away_players: people("equipaFora"),
            officials: v["juizes"]
                .as_array()
                .map(|rows| {
                    rows.iter()
                        .map(|r| format!("{} · {}", text(r, "NOME"), text(r, "FJU_DESCRICAO")))
                        .collect()
                })
                .unwrap_or_default(),
            fetched_at: Some(now()),
        })
    }
}
