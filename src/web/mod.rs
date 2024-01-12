use crate::{
    state::{AppState, Website},
    utils::is_valid_username,
    web::errors::APIError,
};
use axum::{
    body::Body, extract::Request, handler::HandlerWithoutStateExt, response::IntoResponse,
    routing::*, Router,
};

use color_eyre::eyre::Result;
use std::net::SocketAddr;
use tower::{Service, ServiceBuilder, ServiceExt};

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

pub async fn run(state: AppState, addr: SocketAddr) -> Result<()> {
    let admin_router = Router::new()
        .route("/", post(api_admin::is_admin))
        .route("/guestbook", get(api_admin::get_guestbook_requests))
        .route("/guestbook", post(api_admin::approve_guestbook_entry))
        .route("/applications", get(api_admin::get_applications))
        .route("/applications", post(api_admin::approve_application));

    let www_path = std::path::Path::new(&state.config.base_dir)
        .join(&state.config.home_dirs)
        .join("henry")
        .join("dawdle.space");

    let router = Router::new()
        .nest(
            "/api",
            Router::new()
                .nest("/admin", admin_router)
                .route("/chat", get(chat::handler))
                .route("/login", post(api::login))
                .route("/logout", post(api::logout))
                .route("/guestbook", get(api::get_guestbook))
                .route("/guestbook", post(api::add_guestbook_entry))
                .route("/me", get(api::get_me))
                .route("/public_key", post(api::add_public_key))
                .route("/public_key", delete(api::remove_public_key))
                .route("/apply", post(api::apply))
                .route("/claim", post(api::claim))
                .fallback(|| async { APIError::NotFound.into_response() }),
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
    let mut router_service = ServiceBuilder::new().service(router.into_service::<Body>());
    router_service.ready().await?;

    // Use a different service based on the hostname
    let app = |request: Request| async move {
        let hostname_header = request
            .headers()
            .get("HOST")
            .ok_or_else(|| APIError::BadRequest("no hostname".to_string()))?
            .to_str()
            .map_err(|_| APIError::BadRequest("invalid hostname".to_string()))?;

        let site = match select_service(hostname_header) {
            Ok(SelectedService::DawdleSpace) => {
                return APIResult::Ok(router_service.call(request).await.into_response())
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
                if !is_valid_username(username) {
                    return APIResult::Ok(NOT_FOUND.into_response());
                }

                let path = std::path::Path::new(&state.config.base_dir)
                    .join(&state.config.home_dirs)
                    .join(username.to_ascii_lowercase())
                    .join("public");

                let service = create_dir_service(path.clone(), path.join("404.html"), NOT_FOUND);
                let res = service.oneshot(request).await;
                APIResult::Ok(res.into_response())
            }
            _ => APIResult::Ok(NOT_FOUND.into_response()),
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

    let domain = addr::parse_domain_name(hostname)
        .map_err(|_| APIError::BadRequest("invalid hostname".to_string()))?;

    if !cfg!(debug_assertions) && port != "80" {
        return APIResult::Err(APIError::BadRequest("insecure connection".to_string()));
    }

    if is_api(domain) {
        return Ok(SelectedService::DawdleSpace);
    }

    Ok(match is_on_dawdle_space(domain) {
        true => {
            let subdomain = domain
                .prefix()
                .map(|s| s.to_string())
                .ok_or_else(|| APIError::BadRequest("invalid hostname".to_string()))?;
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
