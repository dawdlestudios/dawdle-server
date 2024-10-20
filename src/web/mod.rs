use crate::{
    app::{App, Website},
    web::errors::APIError,
};
use axum::{
    body::Body,
    extract::Request,
    handler::HandlerWithoutStateExt,
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::*,
    Router,
};

use errors::ApiErrorExt;
use eyre::Result;
use std::net::SocketAddr;
use tower::{Service, ServiceBuilder, ServiceExt};
use tower_http::set_header::SetResponseHeaderLayer;

use self::{
    errors::{APIResult, NOT_FOUND},
    files::create_dir_service,
};

mod api;
mod api_admin;
mod chat;
mod errors;
mod files;
mod middleware;
mod webdav;

pub async fn run(state: App, addr: SocketAddr) -> Result<()> {
    let admin_router = Router::new()
        .route("/", post(api_admin::is_admin))
        .route("/applications", get(api_admin::get_applications))
        .route(
            "/applications/approve",
            post(api_admin::approve_application),
        )
        .route(
            "/applications/unapprove",
            post(api_admin::unapprove_application),
        )
        .route(
            "/applications/username",
            post(api_admin::update_application_username),
        )
        .route("/applications", delete(api_admin::delete_application))
        .route("/users", get(api_admin::get_users))
        .route("/user/{username}", delete(api_admin::delete_user));

    let www_path = state
        .config
        .user_home("henry")
        .unwrap()
        .join("sites")
        .join("dawdle.space");

    let router = Router::new()
        .nest(
            "/api",
            Router::new()
                .nest("/admin", admin_router)
                .route("/chat", get(chat::handler))
                .route("/login", post(api::login))
                .route("/logout", post(api::logout))
                .route("/me", get(api::get_me))
                .route("/password", post(api::change_password))
                .route("/minecraft", post(api::update_minecraft_username))
                .route("/public_key", post(api::add_public_key))
                .route("/public_key", delete(api::remove_public_key))
                .route("/apply", post(api::apply))
                .route("/claim", post(api::claim))
                .route("/sites", get(api::get_sites))
                .fallback(|| async {
                    APIError::new(StatusCode::NOT_FOUND, "not found").into_response()
                }),
        )
        .route("/api/webdav", any(webdav::handler))
        .route("/api/webdav/", any(webdav::handler))
        .route("/api/webdav/*rest", any(webdav::handler))
        .fallback_service(create_dir_service(
            www_path.clone(),
            www_path.join("404.html"),
            NOT_FOUND,
        ))
        .with_state(state.clone());

    // only construct the router service once
    let mut router_service = ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::if_not_present(
            header::SERVER,
            HeaderValue::from_static("dawdle.space"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_XSS_PROTECTION,
            HeaderValue::from_static("1; mode=block"),
        ))
        .service(router.into_service::<Body>());

    router_service.ready().await?;

    // Use a different service based on the hostname
    let app = |request: Request| async move {
        let hostname_header = request
            .headers()
            .get("HOST")
            .api_error(StatusCode::BAD_REQUEST, Some("no hostname"))?
            .to_str()
            .api_error(StatusCode::BAD_REQUEST, Some("invalid hostname"))?;

        let site = match select_service(hostname_header) {
            Ok(SelectedService::DawdleSpace) => {
                return APIResult::Ok(router_service.call(request).await.into_response());
            }
            Ok(SelectedService::Subdomain(subdomain)) => state.sites.get(&subdomain),
            Ok(SelectedService::CustomDomain(hostname)) => state.sites.get(&hostname),
            Err(err) => return APIResult::Err(err),
        };

        let Some(site) = site else {
            return APIResult::Ok(NOT_FOUND.into_response());
        };

        match site.value() {
            Website::User(username) => {
                let path = state.config.user_public_path(username).api_not_found()?;
                let service = create_dir_service(path.clone(), path.join("404.html"), NOT_FOUND);
                let res = service.oneshot(request).await;
                APIResult::Ok(res.into_response())
            }
            Website::Site(username, path) => {
                let path = state.config.project_path(username, path).api_not_found()?;
                let service = create_dir_service(path.clone(), path.join("404.html"), NOT_FOUND);
                let res = service.oneshot(request).await;

                APIResult::Ok(res.into_response())
            }
        }
    };

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_service()).await?;

    Ok(())
}

#[derive(Debug)]
enum SelectedService {
    DawdleSpace,
    Subdomain(String),
    CustomDomain(String),
}

fn select_service(hostname_header: &str) -> APIResult<SelectedService> {
    let (hostname, port) = if let Some(colon) = hostname_header.find(':') {
        let (hostname, port) = hostname_header.split_at(colon);
        (hostname, &port[1..])
    } else {
        (hostname_header, "80")
    };

    let Ok(domain) = addr::parse_domain_name(hostname) else {
        return Err(APIError::new(StatusCode::BAD_REQUEST, "invalid hostname"));
    };

    if !cfg!(debug_assertions) && port != "80" {
        return Err(APIError::new(StatusCode::BAD_REQUEST, "invalid port"));
    }

    if is_api(domain) {
        return Ok(SelectedService::DawdleSpace);
    }

    Ok(match is_on_dawdle_space(domain) {
        true => {
            let subdomain = domain
                .prefix()
                .map(|s| s.to_string())
                .api_error(StatusCode::BAD_REQUEST, Some("invalid hostname"))?;
            SelectedService::Subdomain(subdomain)
        }
        false => SelectedService::CustomDomain(hostname.to_string()),
    })
}

fn is_on_dawdle_space(domain: addr::domain::Name) -> bool {
    if cfg!(debug_assertions) {
        (domain.root() == Some("dawdle.localhost") && domain.suffix() == "localhost")
            || (domain.root().is_none() && domain.suffix() == "localhost")
    } else {
        domain.root() == Some("dawdle.space") && domain.suffix() == "space"
    }
}

fn is_api(domain: addr::domain::Name) -> bool {
    if cfg!(debug_assertions) {
        is_on_dawdle_space(domain) && domain.prefix().is_none()
    } else {
        domain.root() == Some("dawdle.space")
            && domain.prefix().is_none()
            && domain.suffix() == "space"
    }
}
