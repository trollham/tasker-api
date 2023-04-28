# tasker-api
`tasker-api` is a toy FizzBuzz implementation, presenting an API to print out either "Fizz", "Buzz", or "Fizz Buzz" after a wait. The system is distributed-friendly, and tasks are persisted in a Postgres database.


## Dependencies
* Access to a Postgres database.
* [sqlx-cli](https://crates.io/crates/sqlx-cli) for running database migrations.

## Getting started

Using a .env file:
```bash
echo DATABASE_URL="postgres://<postgres url>" > .env
sqlx database create && sqlx migrate run
cargo run --release
```

You can also run tasker-api using environment variables rather than a .env file
```bash
export DATABASE_URL="postgres://<postgres url>"
# optionally, run `sqlx database create` to automatically create a 
# database at the above URL
#sqlx database create 
sqlx migrate run
cargo run --release
```

## Usage

Endpoints
- `GET /tasks[/:task_id]`
    - Get a list of tasks. By default, excludes deleted tasks.
    - **Path parameters**
        - `:task_id` - Optional
            - return a single UUID
    - **Query parameters**
        - `types` - Optional
            - comma-delimited list of task types to filter from the result set. Allowable values: `fizz`, `buzz`, `fizzbuzz`
        - `states` - Optional
            - comma-delimited list of task states to filter from the result set. Allowable values: `incomplete`, `complete`, `deleted`
- `DELETE /tasks/:task_id`
    - Mark a task as deleted in the database. Deleted tasks will not be run.
    - **Path parameters**
        - `:task_id` - Required
            - the UUID for the task
- `POST /tasks`
    - Body:
    ```json
        { "task_type": "fizz"|"buzz"|"fizzbuzz" }
    ```
    - Response:
        - UUID of the created task
