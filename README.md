# rust-zero2prod
[![codecov](https://codecov.io/gh/mr-cell/rust-zero2prod/branch/master/graph/badge.svg?token=7G84R9BDAM)](https://codecov.io/gh/mr-cell/rust-zero2prod)

## Building the application
```shell
$ cargo build
```

## Building the Docker image
```shell
$ docker build . -t rust-zero2prod:latest
```
## Running the application locally

### Prerequisites
- psql CLI installed 
- Docker

The application requires a PostgreSQL DB to be working on localhost:5432. 
One can use a dedicated script to start it (Docker is used):
```shell
$ POSTGRES_USER=<db_user> POSTGRES_PASSWORD=<db_password> ./scripts/init_db.sh
```
where:
- **<db_user>** is the PostgreSQL database user
- **<db_password>** is the PostgreSQL database password

To remove the PostgreSQL Docker container one can run:
```shell
$ ./scripts/remove_db.sh
```

### Starting the application
To start the application locally one can execute the following command:
```shell
$ APP_DATABASE__USERNAME=<db_user> APP_DATABASE__PASSWORD=<db_password> APP_EMAIL_CLIENT__API_KEY=<sendgrid_api_key> cargo run
```
where:
- **<sendgrid_api_key>** is the API key for SendGrid email sending service
- **<db_user>** is the PostgreSQL database user
- **<db_password>** is the PostgreSQL database password

## Running the application via Docker Compose

### Prerequisites
- Docker
- Docker Compose

To start the Docker Compose stack one needs to fill in the appropriate .env file with necessary secrets.
A template .env file can be seen here: *./profiles/.env.template*. 
One can create its own ./profiles/.env file based on the template and fill in all necessary secrets (db credentials, api keys).

### Building the application
To build the application one can execute the following command:
```shell
$ docker-compose --env-file=./profiles/.env build
```

### Starting the application
To start the application one can execute the following command:
```shell
$ docker-compose --env-file=./profiles/.env up
```

The command should also build the application (both binary and Docker image) if the Docker image does not exist.

## Adding new SQL migrations
To add new SQL migration one needs to execute the following command (sqlx-cli create needs to be installed):
```shell
$ sqlx migrate add <migration_script_name>
```

An empty file with that name will be created in ./migrations directory. To apply the migration one needs to **rebuild**
the application and run it. Why rebuild? Because the app internally uses *sqlx::migrate!()* macro which gets expanded at the
build time - so migration file list will get set then. **If we add a new migration script after the aplication has been built
and try to run it - the new migration won't be applied**!