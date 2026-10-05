mod store;
use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::{SinkExt, StreamExt};
use pillbox_core::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use store::Store;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer, trace::TraceLayer};
use uuid::Uuid;
type ApiResult<T> = Result<T, ApiError>;
struct ApiError(StatusCode, String);
impl ApiError {
    fn bad(s: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, s.into())
    }
    fn unauthorized() -> Self {
        Self(
            StatusCode::UNAUTHORIZED,
            "Choose a player name first".into(),
        )
    }
    fn forbidden() -> Self {
        Self(StatusCode::FORBIDDEN, "Only the owner can do that".into())
    }
    fn not_found(s: &str) -> Self {
        Self(StatusCode::NOT_FOUND, s.into())
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error=%e,"database operation failed");
        Self(
            StatusCode::SERVICE_UNAVAILABLE,
            "Storage unavailable; please retry".into(),
        )
    }
}
#[derive(Clone, Debug, Serialize)]
struct User {
    id: Uuid,
    name: String,
}
#[derive(Clone)]
struct AppState {
    store: Store,
    rooms: Arc<Mutex<BTreeMap<Uuid, Room>>>,
    results: mpsc::Sender<ResultJob>,
    shutdown: broadcast::Sender<()>,
    closing: Arc<AtomicBool>,
    origin: Option<String>,
    rates: Arc<Mutex<BTreeMap<String, (Instant, u32)>>>,
}
struct Room {
    config: ServerConfig,
    owner: Uuid,
    world: World,
    connections: BTreeMap<String, Uuid>,
    inputs: BTreeMap<String, VecDeque<Input>>,
    tx: broadcast::Sender<String>,
    match_id: Uuid,
    persisted: bool,
    persisting: bool,
    touched: Instant,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ServerConfig {
    name: String,
    visibility: String,
    max_players: usize,
    map_version: Uuid,
    mode_version: Uuid,
}
#[derive(Clone)]
struct ResultJob {
    id: Uuid,
    server: Uuid,
    map: Uuid,
    mode: Uuid,
    result: Value,
    players: Vec<Player>,
}
#[derive(Serialize)]
struct RoomView {
    id: Uuid,
    owner: Uuid,
    config: ServerConfig,
    phase: Phase,
    players: usize,
    match_id: Uuid,
    persisted: bool,
}
impl Room {
    fn view(&self, id: Uuid) -> RoomView {
        RoomView {
            id,
            owner: self.owner,
            config: self.config.clone(),
            phase: self.world.phase,
            players: self.world.players.len(),
            match_id: self.match_id,
            persisted: self.persisted,
        }
    }
}
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "pillbox_server=info,tower_http=info".into()),
        )
        .init();
    if let Err(error) = run().await {
        tracing::error!(%error,"Pillbox failed");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL is required; apply db/schema.sql using Atlas before starting")?;
    let store=Store::connect(&url).await.map_err(|_|"Database connection or schema check failed; verify DATABASE_URL and apply the declarative schema")?;
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".into())
        .parse()?;
    let assets = std::env::var("ASSET_DIR").unwrap_or_else(|_| "web".into());
    let (tx, rx) = mpsc::channel(128);
    let (shutdown, _) = broadcast::channel(1);
    let state = AppState {
        store,
        rooms: Arc::new(Mutex::new(BTreeMap::new())),
        results: tx,
        shutdown,
        closing: Arc::new(AtomicBool::new(false)),
        origin: std::env::var("PUBLIC_ORIGIN")
            .ok()
            .map(|origin| origin.trim().to_owned())
            .filter(|origin| !origin.is_empty()),
        rates: Arc::new(Mutex::new(BTreeMap::new())),
    };
    let (drain, producer_done) = oneshot::channel();
    let writer = tokio::spawn(result_writer(
        state.store.clone(),
        state.rooms.clone(),
        rx,
        producer_done,
    ));
    let ticker = tokio::spawn(tick_loop(state.clone()));
    let app=Router::new().route("/health",get(||async{Json(json!({"status":"ok"}))}))
 .route("/api/session",post(guest).get(me)).route("/api/templates",get(templates))
 .route("/api/content/{kind}",get(list_content).post(save_content)).route("/api/content/{kind}/{id}/publish",post(publish_content))
 .route("/api/servers",get(list_servers).post(create_server)).route("/api/servers/{id}",get(server).put(configure_server))
 .route("/api/servers/{id}/start",post(start)).route("/api/servers/{id}/finish",post(finish))
 .route("/api/servers/{id}/ws",get(upgrade)).route("/api/results/{id}",get(result))
 .fallback_service(ServeDir::new(assets)).layer(DefaultBodyLimit::max(512*1024))
 .layer(SetResponseHeaderLayer::if_not_present(header::X_CONTENT_TYPE_OPTIONS,header::HeaderValue::from_static("nosniff")))
 .layer(SetResponseHeaderLayer::if_not_present(header::REFERRER_POLICY,header::HeaderValue::from_static("same-origin")))
 .layer(SetResponseHeaderLayer::if_not_present(header::CONTENT_SECURITY_POLICY,header::HeaderValue::from_static("default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; connect-src 'self'; img-src 'self' data:; worker-src 'self' blob:; frame-ancestors 'none'")))
 .layer(TraceLayer::new_for_http()).with_state(state.clone());
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, port)).await?;
    tracing::info!(port, "Pillbox listening");
    let stop = state.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            stop.closing.store(true, Ordering::Relaxed);
            let _ = stop.shutdown.send(());
        })
        .await?;
    ticker.await?;
    let _ = drain.send(());
    drop(state);
    tokio::time::timeout(Duration::from_secs(15), writer)
        .await
        .map_err(|_| "Timed out draining match results during shutdown")???;
    Ok(())
}
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler");
        tokio::select! {_=tokio::signal::ctrl_c()=>(),_=term.recv()=>()}
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
fn check_origin(state: &AppState, h: &HeaderMap) -> ApiResult<()> {
    if let Some(origin) = h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        let expected = state.origin.as_deref();
        let host = h
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if expected.is_some_and(|s| s != origin)
            || (expected.is_none()
                && origin != format!("http://{host}")
                && origin != format!("https://{host}"))
        {
            return Err(ApiError::forbidden());
        }
    }
    Ok(())
}
async fn auth(state: &AppState, h: &HeaderMap) -> ApiResult<User> {
    check_origin(state, h)?;
    let token = h
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.split(';')
                .find_map(|c| c.trim().strip_prefix("pillbox_session="))
        })
        .ok_or_else(ApiError::unauthorized)?;
    state.store.authenticate(token).await
}
async fn rate(state: &AppState, key: String, limit: u32) -> ApiResult<()> {
    let mut rates = state.rates.lock().await;
    rates.retain(|_, (t, _)| t.elapsed() < Duration::from_secs(60));
    if rates.len() > 4096 {
        return Err(ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "Try again shortly".into(),
        ));
    }
    let e = rates.entry(key).or_insert((Instant::now(), 0));
    e.1 += 1;
    if e.1 > limit {
        Err(ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "Please slow down".into(),
        ))
    } else {
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Guest {
    name: String,
}
async fn guest(
    State(s): State<AppState>,
    h: HeaderMap,
    Json(g): Json<Guest>,
) -> ApiResult<Response> {
    check_origin(&s, &h)?;
    rate(&s, "guest-global".into(), 60).await?;
    if !valid_name(&g.name) {
        return Err(ApiError::bad("Name must contain 1–64 visible characters"));
    }
    if let Ok(u) = auth(&s, &h).await {
        return Ok(Json(u).into_response());
    }
    let (u, token) = s.store.guest(g.name.trim().into()).await?;
    let secure = s.origin.as_ref().is_some_and(|o| o.starts_with("https://"));
    let cookie = format!(
        "pillbox_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=2592000{}",
        if secure { "; Secure" } else { "" }
    );
    Ok(([(header::SET_COOKIE, cookie)], Json(u)).into_response())
}
async fn me(State(s): State<AppState>, h: HeaderMap) -> ApiResult<Json<User>> {
    Ok(Json(auth(&s, &h).await?))
}
async fn templates() -> Json<Value> {
    Json(
        json!({"map":starter_map(),"modes":[Mode::builtin("Free For All",Objective::None,false),Mode::builtin("Team Deathmatch",Objective::None,true),Mode::builtin("Capture the Flag",Objective::Flags,true),Mode::builtin("King of the Hill",Objective::Hill,true)]}),
    )
}
async fn list_content(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(kind): Path<String>,
) -> ApiResult<Json<Vec<store::Version>>> {
    let u = auth(&s, &h).await?;
    Ok(Json(s.store.list(&kind, u.id).await?))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    content_id: Option<Uuid>,
    document: Value,
}
async fn save_content(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(kind): Path<String>,
    Json(body): Json<Save>,
) -> ApiResult<Json<store::Version>> {
    let u = auth(&s, &h).await?;
    rate(&s, format!("save:{}", u.id), 30).await?;
    match kind.as_str() {
        "maps" => {
            let m: Map = serde_json::from_value(body.document.clone())
                .map_err(|e| ApiError::bad(e.to_string()))?;
            m.validate().map_err(ApiError::bad)?;
        }
        "modes" => {
            let m: Mode = serde_json::from_value(body.document.clone())
                .map_err(|e| ApiError::bad(e.to_string()))?;
            m.validate().map_err(ApiError::bad)?;
        }
        _ => return Err(ApiError::not_found("Unknown content type")),
    }
    Ok(Json(
        s.store
            .save(&kind, u.id, body.content_id, body.document)
            .await?,
    ))
}
async fn publish_content(
    State(s): State<AppState>,
    h: HeaderMap,
    Path((kind, id)): Path<(String, Uuid)>,
) -> ApiResult<Json<Value>> {
    let u = auth(&s, &h).await?;
    rate(&s, format!("publish:{}", u.id), 30).await?;
    s.store.publish(&kind, id, u.id).await?;
    Ok(Json(json!({"published":true})))
}
fn validate_config(c: &ServerConfig) -> ApiResult<()> {
    if !valid_name(&c.name)
        || !(2..=16).contains(&c.max_players)
        || !matches!(c.visibility.as_str(), "public" | "unlisted")
    {
        return Err(ApiError::bad("Invalid server settings"));
    }
    Ok(())
}
async fn load_world(s: &AppState, c: &ServerConfig, user: Uuid) -> ApiResult<World> {
    validate_config(c)?;
    let map = s.store.get("maps", c.map_version, user).await?;
    let mode = s.store.get("modes", c.mode_version, user).await?;
    if !map.published || !mode.published {
        return Err(ApiError::bad("Publish map and mode before hosting"));
    }
    World::new(
        serde_json::from_value(map.document).map_err(|_| ApiError::bad("Invalid stored map"))?,
        serde_json::from_value(mode.document).map_err(|_| ApiError::bad("Invalid stored mode"))?,
    )
    .map_err(ApiError::bad)
}
async fn create_server(
    State(s): State<AppState>,
    h: HeaderMap,
    Json(c): Json<ServerConfig>,
) -> ApiResult<Json<RoomView>> {
    let u = auth(&s, &h).await?;
    rate(&s, format!("create:{}", u.id), 10).await?;
    let world = load_world(&s, &c, u.id).await?;
    let mut rooms = s.rooms.lock().await;
    if rooms.len() >= 64 || rooms.values().filter(|r| r.owner == u.id).count() >= 4 {
        return Err(ApiError::bad(
            "Server limit reached; reuse an existing server",
        ));
    }
    let id = Uuid::new_v4();
    let (tx, _) = broadcast::channel(4);
    let r = Room {
        config: c,
        owner: u.id,
        world,
        connections: BTreeMap::new(),
        inputs: BTreeMap::new(),
        tx,
        match_id: Uuid::new_v4(),
        persisted: false,
        persisting: false,
        touched: Instant::now(),
    };
    let v = r.view(id);
    rooms.insert(id, r);
    Ok(Json(v))
}
async fn configure_server(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
    Json(c): Json<ServerConfig>,
) -> ApiResult<Json<RoomView>> {
    let u = auth(&s, &h).await?;
    let world = load_world(&s, &c, u.id).await?;
    let mut rooms = s.rooms.lock().await;
    let r = rooms
        .get_mut(&id)
        .ok_or_else(|| ApiError::not_found("Server not found"))?;
    if r.owner != u.id {
        return Err(ApiError::forbidden());
    }
    if !r.connections.is_empty()
        || !matches!(r.world.phase, Phase::Waiting | Phase::Finished)
        || (r.world.phase == Phase::Finished && !r.persisted)
    {
        return Err(ApiError::bad(
            "Wait for results to save and all players to leave before reconfiguring",
        ));
    }
    r.config = c;
    r.world = world;
    r.touched = Instant::now();
    Ok(Json(r.view(id)))
}
async fn list_servers(State(s): State<AppState>, h: HeaderMap) -> Json<Vec<RoomView>> {
    let owner = if h.contains_key(header::COOKIE) {
        auth(&s, &h).await.ok().map(|u| u.id)
    } else {
        None
    };
    Json(
        s.rooms
            .lock()
            .await
            .iter()
            .filter(|(_, r)| r.config.visibility == "public" || owner == Some(r.owner))
            .map(|(id, r)| r.view(*id))
            .collect(),
    )
}
async fn server(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    auth(&s, &h).await?;
    let rooms = s.rooms.lock().await;
    let r = rooms
        .get(&id)
        .ok_or_else(|| ApiError::not_found("Server not found"))?;
    Ok(Json(
        json!({"server":r.view(id),"map":r.world.map,"mode":r.world.mode,"state":r.world.snapshot()}),
    ))
}
async fn start(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let u = auth(&s, &h).await?;
    let mut rooms = s.rooms.lock().await;
    let r = rooms
        .get_mut(&id)
        .ok_or_else(|| ApiError::not_found("Server not found"))?;
    if r.owner != u.id {
        return Err(ApiError::forbidden());
    }
    if r.world.phase == Phase::Finished && !r.persisted {
        return Err(ApiError::bad("Results are still saving"));
    }
    r.world.start().map_err(ApiError::bad)?;
    r.match_id = Uuid::new_v4();
    r.persisted = false;
    r.persisting = false;
    for q in r.inputs.values_mut() {
        q.clear();
    }
    Ok(Json(json!({"match_id":r.match_id})))
}
async fn finish(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Value>> {
    let u = auth(&s, &h).await?;
    let mut rooms = s.rooms.lock().await;
    let r = rooms
        .get_mut(&id)
        .ok_or_else(|| ApiError::not_found("Server not found"))?;
    if r.owner != u.id {
        return Err(ApiError::forbidden());
    }
    if !matches!(r.world.phase, Phase::Playing | Phase::Starting) {
        return Err(ApiError::bad("No active match"));
    }
    r.world.finish();
    Ok(Json(json!({"finished":true})))
}
async fn result(State(s): State<AppState>, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    Ok(Json(s.store.result(id).await?))
}
async fn upgrade(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> ApiResult<Response> {
    let u = auth(&s, &h).await?;
    rate(&s, format!("connect:{}", u.id), 20).await?;
    if s.closing.load(Ordering::Relaxed) {
        return Err(ApiError::bad("Server shutting down"));
    }
    let rooms = s.rooms.lock().await;
    let r = rooms
        .get(&id)
        .ok_or_else(|| ApiError::not_found("Server not found"))?;
    if r.connections.contains_key(&u.id.to_string())
        || r.world.players.len() >= r.config.max_players
    {
        return Err(ApiError::bad("Already joined or server full"));
    }
    drop(rooms);
    Ok(ws
        .max_message_size(2048)
        .max_frame_size(2048)
        .on_upgrade(move |socket| connection(s, id, u, socket)))
}
async fn connection(s: AppState, id: Uuid, user: User, socket: WebSocket) {
    let uid = user.id.to_string();
    let connection_id = Uuid::new_v4();
    let mut stop = s.shutdown.subscribe();
    let mut rx = {
        let mut rooms = s.rooms.lock().await;
        let Some(r) = rooms.get_mut(&id) else {
            return;
        };
        if r.world.players.len() >= r.config.max_players
            || r.connections.contains_key(&uid)
            || r.world.join(uid.clone(), user.name).is_err()
        {
            return;
        }
        r.connections.insert(uid.clone(), connection_id);
        r.inputs.insert(uid.clone(), VecDeque::new());
        r.touched = Instant::now();
        r.tx.subscribe()
    };
    let (mut sink, mut source) = socket.split();
    let mut idle = Instant::now();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    let mut count = 0_u32;
    let mut window = Instant::now();
    let mut seq = 0;
    loop {
        tokio::select! {
         _=stop.recv()=>{let _=sink.send(Message::Close(None)).await;break;},
         _=heartbeat.tick()=>{if idle.elapsed()>Duration::from_secs(15){break;}},
         packet=rx.recv()=>{match packet{Ok(data)=>if !matches!(tokio::time::timeout(Duration::from_secs(2),sink.send(Message::Text(data.into()))).await,Ok(Ok(()))){break;},Err(broadcast::error::RecvError::Lagged(_))=>continue,Err(_)=>break}},
         message=source.next()=>{match message{
          Some(Ok(Message::Text(text)))=>{idle=Instant::now();if window.elapsed()>=Duration::from_secs(1){window=Instant::now();count=0;}count+=1;if count>120{break;}
           let Ok(input)=serde_json::from_str::<Input>(&text)else{break;};if !input.valid()||input.seq<=seq||input.seq>seq+600{break;}seq=input.seq;
           let mut rooms=s.rooms.lock().await;let Some(r)=rooms.get_mut(&id)else{break;};if r.world.phase==Phase::Playing{let q=r.inputs.get_mut(&uid).unwrap();if q.len()<12{q.push_back(input);}}
          },Some(Ok(Message::Ping(data)))=>{idle=Instant::now();if sink.send(Message::Pong(data)).await.is_err(){break;}},Some(Ok(Message::Pong(_)))=>{idle=Instant::now();},_=>break,
         }}
        }
    }
    let mut rooms = s.rooms.lock().await;
    if let Some(r) = rooms.get_mut(&id)
        && r.connections.get(&uid) == Some(&connection_id)
    {
        r.connections.remove(&uid);
        r.inputs.remove(&uid);
        r.world.leave(&uid);
        r.touched = Instant::now();
    }
}
async fn tick_loop(s: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs_f64(1. / 60.));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let closing = s.closing.load(Ordering::Relaxed);
        let mut rooms = s.rooms.lock().await;
        for (id, r) in rooms.iter_mut() {
            if closing && matches!(r.world.phase, Phase::Starting | Phase::Playing) {
                r.world.finish();
            }
            let inputs = r
                .inputs
                .iter_mut()
                .filter_map(|(id, q)| q.pop_front().map(|i| (id.clone(), i)))
                .collect();
            r.world.step(&inputs);
            if r.world.phase == Phase::Finished && !r.persisting && !r.persisted {
                let mut players: Vec<_> = r.world.players.values().cloned().collect();
                players.extend(r.world.departed.clone());
                let job = ResultJob {
                    id: r.match_id,
                    server: *id,
                    map: r.config.map_version,
                    mode: r.config.mode_version,
                    result: json!({"match_id":r.match_id,"server_id":id,"map_version":r.config.map_version,"mode_version":r.config.mode_version,"state":r.world.snapshot(),"participants":players}),
                    players,
                };
                if s.results.try_send(job).is_ok() {
                    r.persisting = true;
                }
            }
            if r.world.tick.is_multiple_of(3) {
                let data=json!({"state":r.world.snapshot(),"match_id":r.match_id,"persisted":r.persisted}).to_string();
                let _ = r.tx.send(data);
            }
        }
        rooms.retain(|_, r| {
            !r.connections.is_empty()
                || r.touched.elapsed() < Duration::from_secs(600)
                || r.persisting
                || (r.world.phase == Phase::Finished && !r.persisted)
        });
        if closing {
            break;
        }
    }
}
async fn result_writer(
    store: Store,
    rooms: Arc<Mutex<BTreeMap<Uuid, Room>>>,
    mut rx: mpsc::Receiver<ResultJob>,
    mut producer_done: oneshot::Receiver<()>,
) -> Result<(), &'static str> {
    // Only the ticker produces results. Close/drain after it exits; HTTP state
    // can retain sender clones even after the listener stops accepting requests.
    let mut failed = BTreeSet::new();
    let mut draining = false;
    loop {
        let job = tokio::select! {
            _ = &mut producer_done, if !draining => {
                rx.close();
                draining = true;
                continue;
            }
            job = rx.recv() => job,
        };
        let Some(job) = job else { break };
        let mut saved = false;
        for attempt in 0..3 {
            match store.persist(&job).await {
                Ok(()) => {
                    saved = true;
                    break;
                }
                Err(e) => {
                    tracing::error!(error=%e,match_id=%job.id,"result persistence failed");
                    tokio::time::sleep(Duration::from_millis(200 * (attempt + 1))).await;
                }
            }
        }
        if saved {
            failed.remove(&job.id);
        } else {
            failed.insert(job.id);
        }
        if let Some(r) = rooms.lock().await.get_mut(&job.server)
            && r.match_id == job.id
        {
            r.persisted = saved;
            r.persisting = false;
        }
    }
    if !failed.is_empty() {
        Err("One or more match results could not be persisted")
    } else {
        Ok(())
    }
}
