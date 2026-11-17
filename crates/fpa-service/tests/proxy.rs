use axum::{
    Router,
    http::{StatusCode, Uri},
    response::IntoResponse,
};
use fpa_proto::v1::{self as pb, results_service_server::ResultsService};
use fpa_service::{Proxy, ProxyOptions};
use std::time::Duration;
use tonic::Request;
async fn handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path();
    let (status, body) = match path {
        "/ws/getResultadosCalendario/10019/28412" => {
            (StatusCode::CREATED, include_str!("fixtures/fixtures.json"))
        }
        "/ws/getClassificacoes/10019/28412" => {
            (StatusCode::CREATED, include_str!("fixtures/standings.json"))
        }
        "/ws/getJogo/329208" => (StatusCode::CREATED, include_str!("fixtures/match.json")),
        "/ws/getJogo/999" => (StatusCode::CREATED, include_str!("fixtures/missing.json")),
        "/portal/season" => (StatusCode::OK, include_str!("fixtures/seasons.json")),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "<html>source down</html>"),
    };
    (
        status,
        [
            ("content-type", "application/json"),
            ("x-wp-totalpages", "1"),
        ],
        body,
    )
}
#[tokio::test]
async fn generated_http_source_to_protobuf_missing_status_and_namespaces() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, Router::new().fallback(handler))
            .await
            .unwrap();
    });
    let proxy = Proxy::new(ProxyOptions {
        upstream_base_url: format!("{origin}/ws").parse().unwrap(),
        portal_base_url: format!("{origin}/portal").parse().unwrap(),
        upstream_timeout: Duration::from_secs(2),
        max_concurrent_fetches: 4,
        max_response_bytes: 32 * 1024 * 1024,
    })
    .unwrap();
    let seasons = proxy
        .list_seasons(Request::new(pb::ListSeasonsRequest {}))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(seasons.seasons[0].id, "374");
    let phase = proxy
        .get_phase(Request::new(pb::GetPhaseRequest {
            competition_id: "10019".into(),
            phase_id: "28412".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert!(phase.matches.iter().any(|g| g.home_goals.is_none()));
    assert!(phase.matches.iter().any(|g| g.home_goals == Some(22)));
    assert!(phase.matches.iter().all(|g| g.source_state.is_empty()));
    assert!(!phase.standings.is_empty());
    let sheet = proxy
        .get_match(Request::new(pb::GetMatchRequest {
            match_id: "329208".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(sheet.r#match.unwrap().home_goals, Some(22));
    assert!(!sheet.home_players.is_empty());
    assert_eq!(
        proxy
            .get_match(Request::new(pb::GetMatchRequest {
                match_id: "999".into()
            }))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::NotFound
    );
    assert_eq!(
        proxy
            .get_match(Request::new(pb::GetMatchRequest {
                match_id: "../1".into()
            }))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::InvalidArgument
    );
    assert_eq!(
        proxy
            .list_phases(Request::new(pb::ListPhasesRequest {
                competition_id: "42".into()
            }))
            .await
            .unwrap_err()
            .code(),
        tonic::Code::Unavailable
    );
    task.abort();
}
