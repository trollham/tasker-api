use core::fmt;

use std::{env, str::FromStr};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

#[derive(Clone, Debug)]
struct SharedState {
    pool: PgPool,
}

#[derive(Clone, Debug, Deserialize, Serialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
enum TaskType {
    Fizz,
    Buzz,
    FizzBuzz,
}

impl TaskType {
    pub fn to_db_value(&self) -> &str {
        match self {
            TaskType::Fizz => "fizz",
            TaskType::Buzz => "buzz",
            TaskType::FizzBuzz => "fizzbuzz",
        }
    }
    pub fn filter_str_vec() -> Vec<String> {
        use TaskType::*;

        vec![Fizz, Buzz, FizzBuzz]
            .into_iter()
            .map(|v| v.to_db_value().to_owned())
            .collect()
    }
}

impl fmt::Display for TaskType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TaskType::Fizz => "Fizz",
            TaskType::Buzz => "Buzz",
            TaskType::FizzBuzz => "Fizz Buzz",
        };
        write!(f, "{s}")
    }
}

impl Into<Duration> for &TaskType {
    fn into(self) -> Duration {
        match self {
            TaskType::Fizz => Duration::seconds(3),
            TaskType::Buzz => Duration::seconds(5),
            TaskType::FizzBuzz => Duration::seconds(0),
        }
    }
}

impl FromStr for TaskType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use TaskType::*;
        match s {
            "fizz" => Ok(Fizz),
            "buzz" => Ok(Buzz),
            "fizzbuzz" => Ok(FizzBuzz),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(rename_all = "snake_case")]
enum TaskState {
    Incomplete,
    Deleted,
    Complete,
}

impl TaskState {
    pub fn filter_str_vec() -> Vec<String> {
        use TaskState::*;

        // Deleted is omitted purposefully to not return them by default
        vec![Incomplete, Complete]
            .into_iter()
            .map(|v| v.to_string())
            .collect()
    }
}

impl FromStr for TaskState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use TaskState::*;
        match s {
            "incomplete" => Ok(Incomplete),
            "deleted" => Ok(Deleted),
            "complete" => Ok(Complete),
            _ => Err(()),
        }
    }
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TaskState::Incomplete => "incomplete",
            TaskState::Deleted => "deleted",
            TaskState::Complete => "complete",
        };
        write!(f, "{s}")
    }
}

enum Error {
    Db(String),
}

impl From<sqlx::Error> for Error {
    fn from(value: sqlx::Error) -> Self {
        Error::Db(value.to_string())
    }
}

#[derive(Debug, Serialize)]
struct ErrorJson {
    message: String,
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let res = match self {
            Error::Db(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorJson { message: e }),
            ),
        };
        res.into_response()
    }
}

#[derive(Debug, Deserialize, Serialize, sqlx::FromRow)]
struct Task {
    id: Uuid,
    task_type: TaskType,
    submitted: DateTime<Utc>,
    state: TaskState,
}

impl Task {
    pub fn handle(self) -> Option<Uuid> {
        let now = Utc::now();
        let prefix = self.task_type.to_string();
        let duration: Duration = (&self.task_type).into();

        let time_passed = now - self.submitted;
        if time_passed < duration {
            None
        } else {
            let postfix = match self.task_type {
                TaskType::Fizz | TaskType::Buzz => self.id.to_string(),
                TaskType::FizzBuzz => now.to_string(),
            };
            println!("{prefix} {postfix}");
            Some(self.id)
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct CreateTaskPayload {
    task_type: TaskType,
}

#[derive(Debug, Deserialize, Serialize)]
struct Filter {
    types: Option<String>,
    states: Option<String>,
}

async fn tasks(
    state: State<SharedState>,
    Query(filter): Query<Filter>,
) -> Result<Json<Vec<Task>>, Error> {
    let Filter { types, states } = filter;
    let types = types
        .map(|s| {
            s.split(",")
                .filter_map(|s| TaskType::from_str(s).ok())
                .map(|v| v.to_db_value().to_owned())
                .collect::<Vec<String>>()
        })
        .unwrap_or_else(|| TaskType::filter_str_vec());

    let states = states
        .map(|s| {
            s.split(",")
                .filter_map(|s| TaskState::from_str(s).ok())
                .map(|v| v.to_string())
                .collect::<Vec<String>>()
        })
        .unwrap_or_else(|| TaskState::filter_str_vec());

    let tasks = sqlx::query_as!(
        Task,
        r#"SELECT id,
                 task_type as "task_type: TaskType", 
                 submitted ,
                 state as "state: TaskState"
            FROM tasks 
            WHERE state = ANY($1)
            AND task_type = ANY($2)
            ORDER BY submitted DESC"#,
        &states[..],
        &types[..]
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(tasks))
}

async fn create_task(
    State(state): State<SharedState>,
    Json(task): axum::Json<CreateTaskPayload>,
) -> Result<String, Error> {
    let uuid = Uuid::new_v4();
    let now = Utc::now();
    let task_type = task.task_type.to_db_value();
    sqlx::query!(
        "INSERT INTO tasks (id, task_type, submitted) VALUES ($1, $2, $3)",
        uuid,
        task_type,
        now
    )
    .execute(&state.pool)
    .await?;
    Ok(uuid.hyphenated().to_string())
}

async fn task(
    Path(filter): Path<Uuid>,
    state: State<SharedState>,
) -> Result<(StatusCode, Json<Option<Task>>), Error> {
    let task = sqlx::query_as!(
        Task,
        r#"
    SELECT id,
        task_type as "task_type: TaskType",
        submitted,
        state as "state: TaskState"
    FROM tasks
    WHERE id = $1
        "#,
        filter
    )
    .fetch_optional(&state.pool)
    .await?;

    if task.is_none() {
        Ok((StatusCode::NOT_FOUND, Json(task)))
    } else {
        Ok((StatusCode::OK, Json(task)))
    }
}

async fn delete_task(
    Path(filter): Path<Uuid>,
    state: State<SharedState>,
) -> Result<StatusCode, Error> {
    let deleted = sqlx::query!(
        "UPDATE tasks SET state = 'deleted' WHERE state = 'incomplete' AND id = $1 RETURNING id",
        filter
    )
    .fetch_optional(&state.pool)
    .await?;

    match deleted {
        Some(id) => {
            eprintln!("deleted {}", id.id.to_string());
            Ok(StatusCode::OK)
        }
        None => Ok(StatusCode::NOT_FOUND),
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let db_url = env::var("DATABASE_URL").unwrap_or("postgres://localhost/tasks".to_string());

    let conn_pool = PgPoolOptions::new().connect(&db_url).await.unwrap();

    // This is safe to clone and pass around threads because the underlyign PgPool is simply a wrapper around an Arc
    let state = SharedState {
        pool: conn_pool.clone(),
    };

    let app = Router::new()
        .route("/tasks", get(tasks).post(create_task))
        .route("/tasks/:task_id", get(task).delete(delete_task))
        .with_state(state);

    let task_handler = tokio::spawn(async move {
        loop {
            let mut transaction = conn_pool.begin().await.expect("connection failed");
            // Query for tasks that have yet to been completed. This query grabs a row lock on the
            // each row in the batch returned, preventing other nodes from returning them. This
            // lock is released when the transaction is dropped or committed. If this loop exits
            // for any reason, then the transaction is rolled back and the rows are free to be
            // picked up by other worker nodes.
            let tasks = sqlx::query_as!(
                Task,
                r#"
                    SELECT 
                        id, 
                        submitted, 
                        state as "state: TaskState", 
                        task_type as "task_type: TaskType"
                    FROM tasks 
                    WHERE state = 'incomplete' 
                    ORDER BY submitted DESC 
                    LIMIT 5 
                    FOR UPDATE SKIP LOCKED
                "#
            )
            .fetch_all(&mut transaction)
            .await
            .unwrap();

            let mut completed_ids = Vec::new();

            for task in tasks {
                let id = task.handle();
                completed_ids.push(id);
            }
            for id in completed_ids {
                // finish up our work on the rows before allowing them to be visible again.
                sqlx::query!("UPDATE tasks SET state = 'complete' WHERE id = $1", id)
                    .execute(&mut transaction)
                    .await
                    .unwrap();
            }
            transaction.commit().await.unwrap();
        }
    });

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
    task_handler.await.unwrap();
}
